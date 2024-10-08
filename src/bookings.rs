use chrono::{Datelike, DateTime, Days, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use rocket::futures::StreamExt;
use rocket::futures::stream::BoxStream;
use rocket::http::Status;
use rocket::response::status::{Created, Custom, NoContent};
use rocket::serde::json::Json;
use rocket::serde::Serialize;
use rocket::State;
use serde::Deserialize;
use sqlx::{Error, Executor, FromRow, PgPool, query_as, QueryBuilder, raw_sql, Row};
use sqlx::postgres::{PgQueryResult, PgRow};

use crate::{AppState, parse_opt_date, SessionLocation, SessionType, UserLoginRecord};
use crate::claims::Claims;

const ROLE_ADMIN: &str = "admin";
const ROLE_FULL_MEMBER: &str = "member";
const ROLE_LIMITED_MEMBER: &str = "limited-member";

#[derive(Serialize, Deserialize, FromRow, Debug, Clone)]
pub struct SessionBooking {
    person_id: i64,
    session_id: i64,
    credits_used: Option<i16>
}

#[derive(Serialize, Debug)]
pub struct SessionBookingFull {
    person_id: i64,
    person_name: String,
    person_email: String,
    session_id: i64,
    session_datetime: DateTime<Utc>,
    session_duration_mins: i32,
    session_location: Option<SessionLocation>,
    session_type: SessionType,
    attended: bool,
    credits_used: i16
}

impl FromRow<'_, PgRow> for SessionBookingFull {
    fn from_row(row: &'_ PgRow) -> Result<Self, Error> {
        let location_id: Option<i32> = row.try_get("session_location_id").ok();
        let location: Option<SessionLocation> = match location_id {
            Some(id) => Some(SessionLocation{
                id,
                name: row.try_get("session_location_name")?,
                address: row.try_get("session_location_address")?,
            }),
            None => None
        };

        Ok(SessionBookingFull {
            person_id: row.try_get("person_id")?,
            person_name: row.try_get("person_name")?,
            person_email: row.try_get("person_email")?,
            session_id: row.try_get("session_id")?,
            session_datetime: row.try_get("session_datetime")?,
            session_duration_mins: row.try_get("session_duration_mins")?,
            session_location: location,
            session_type: SessionType{
                id: row.try_get("session_type_id")?,
                name: row.try_get("session_type_name")?,
                requires_trainer: row.try_get("session_type_requires_trainer").ok().unwrap_or(true),
                cost: row.try_get("session_type_cost")?
            },
            attended: row.try_get("attended").ok().unwrap_or(false),
            credits_used: row.try_get("credits_used")?
        })
    }
}

#[get("/bookings?<session_id>&<person_id>&<from>&<to>")]
pub async fn list_bookings(
    state: &State<AppState>,
    claim: Claims,
    session_id: Option<i64>,
    person_id: Option<i64>,
    from: Option<String>,
    to: Option<String>
) -> Result<Json<Vec<SessionBookingFull>>, Custom<String>> {
    _list_bookings(&state.pool, &claim, session_id, person_id, from, to).await
}

async fn _list_bookings(
    pool: &PgPool,
    claim: &Claims,
    session_id: Option<i64>,
    person_id: Option<i64>,
    from: Option<String>,
    to: Option<String>
) -> Result<Json<Vec<SessionBookingFull>>, Custom<String>> {
    let mut qb = QueryBuilder::new("SELECT b.person_id, p.name AS person_name, p.email AS person_email, b.session_id, b.credits_used, \
                s.datetime AS session_datetime, s.duration_mins AS session_duration_mins, s.location AS session_location_id, l.name AS session_location_name, l.address AS session_location_address, \
                s.session_type AS session_type_id, t.name AS session_type_name, t.requires_trainer AS session_type_requires_trainer, t.cost AS session_type_cost, b.attended \
            FROM booking as b \
            JOIN person AS p ON b.person_id = p.id \
            JOIN session AS s ON b.session_id = s.id \
            JOIN session_type AS t ON s.session_type = t.id \
            LEFT JOIN location AS l ON s.location = l.id ");

    let mut where_op = String::from(" WHERE");

    if let Some(person_id) = person_id {
        if person_id != claim.uid && !claim.has_role("admin") {
            return Err(Custom(Status::Forbidden, "only admins can view bookings for other users".to_string()))
        }
        qb.push(where_op + " b.person_id = ");
        qb.push_bind(person_id);
        where_op = String::from(" AND");
    } else if !claim.has_role("admin") {
        return Err(Custom(Status::Forbidden, "only admins can view bookings for other users".to_string()))
    }

    if let Some(session_id) = session_id {
        qb.push(where_op + " b.session_id = ");
        qb.push_bind(session_id);
        where_op = String::from(" AND");
    }
    if let Some(from) = parse_opt_date(from)? {
        qb.push(where_op + " s.datetime >= ");
        qb.push_bind(from);
        where_op = String::from(" AND");
    }
    if let Some(to) = parse_opt_date(to)? {
        qb.push(where_op + " s.datetime <= ");
        qb.push_bind(to);
    }

    qb.push(" ORDER BY session_datetime, person_name");
    info!("list_bookings compiled SQL: {}", qb.sql());
    let bookings = qb.build_query_as()
        .fetch_all(pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    Ok(Json(bookings))
}

async fn take_result_from_stream<'a>(stream: &mut BoxStream<'a, Result<PgQueryResult, Error>>) -> Result<PgQueryResult, Custom<String>> {
    stream.next()
        .await
        .ok_or(Custom(Status::InternalServerError, "no more results".to_string()))?
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
}

#[post("/bookings", data="<booking>")]
pub async fn create_booking(state: &State<AppState>, claim: Claims, booking: Json<SessionBooking>) -> Result<Created<Json<SessionBooking>>, Custom<String>> {
    _create_booking(&state.pool, &state.timezone, &claim, booking).await
}

async fn _create_booking(pool: &PgPool, timezone: &Tz, claim: &Claims, booking: Json<SessionBooking>) -> Result<Created<Json<SessionBooking>>, Custom<String>> {
    let mut credits_cost: i16 = 0;

    // Admins can always make a booking for any user
    if !claim.has_role(ROLE_ADMIN) {
        // Non-admins can only book on their own behalf
        if claim.uid != booking.person_id {
            info!("person id {} attempted to book session on behalf of person id {}; denied: missing admin role", claim.uid, booking.person_id);
            return Err(Custom(Status::Forbidden, "Cannot create a booking for another user!".to_string()));
        }

        // Non-admins can only book future sessions
        let session_date_and_cost = get_session_date_and_cost(pool, &booking.session_id).await?;
        if session_date_and_cost.datetime.lt(&Utc::now()) {
            info!("person id {} attempted to book session in past (session id {}, date {}); denied: missing admin role", claim.uid, session_date_and_cost.id, session_date_and_cost.datetime);
            return Err(Custom(Status::Forbidden, "Cannot create booking in the past!".to_string()));
        }

        // Check whether the user has full membership or a usable limited membership
        let membership_check: Result<(), Custom<String>>;
        if claim.has_role(ROLE_FULL_MEMBER) {
            membership_check = Ok(());
        } else if claim.has_role(ROLE_LIMITED_MEMBER) {
            membership_check = check_limited_member_has_no_bookings_in_same_week(pool, timezone, claim.uid, &session_date_and_cost).await;
        } else {
            info!("person id {} attempted to book session id {} (cost {}) without active membership or PAYG credits", claim.uid, session_date_and_cost.id, session_date_and_cost.cost);
            membership_check = Err(Custom(Status::Forbidden, "Missing or expired membership, and no PAYG credits.".to_string()));
        }

        // If no usable membership, check for credits
        if membership_check.is_err() && membership_check.as_ref().err().unwrap().0 == Status::Forbidden {
            let user_record = UserLoginRecord::load_by_id(pool, booking.person_id).await
                .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
                .ok_or(Custom(Status::Unauthorized, "missing user record".to_string()))?;
            if user_record.credits >= session_date_and_cost.cost {
                if booking.credits_used.unwrap_or(0) < session_date_and_cost.cost {
                    return Err(Custom(Status::PaymentRequired, "Opt in to use credits for booking.".to_string()));
                } else {
                    credits_cost = session_date_and_cost.cost;
                }
            } else {
                membership_check?;
            }
        } else {
            // Technical errors other than forbidden should break out
            membership_check?;
        }
    }

    // Read the max_booking_count for the session if present
    let session_with_max_booking_count: SessionWithMaxBookingCount = query_as("SELECT id, max_booking_count FROM session WHERE id = $1")
        .bind(&booking.session_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or(Custom(Status::NotFound, format!("no session with id {}", &booking.session_id)))?;

    // Make the booking
    match session_with_max_booking_count.max_booking_count {
        Some(max_booking_count) => book_session_with_max_bookings(pool, booking.person_id, booking.session_id, max_booking_count, credits_cost).await,
        None => book_session_no_max_bookings(pool, booking.person_id, booking.session_id, credits_cost).await
    }?;

    info!("Created booking: {:?}", &booking);

    // Debit the credits used from the user if required
    if credits_cost > 0 {
        query_as("UPDATE person SET credits = credits - $1 WHERE id = $2 RETURNING id, credits")
            .bind(credits_cost)
            .bind(booking.person_id)
            .fetch_one(pool)
            .await.map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    }

    Ok(Created::new(format!("/bookings?sessionid={},person_id={}", booking.session_id, booking.person_id)))
}

#[derive(FromRow)]
pub struct SessionDateAndCost {
    id: i64,
    datetime: DateTime<Utc>,
    cost: i16
}

#[derive(FromRow, Debug)]
struct MemberExistingBooking {
    person_id: i64,
    session_id: i64,
    datetime: DateTime<Utc>
}

async fn check_limited_member_has_no_bookings_in_same_week(pool: &PgPool, timezone: &Tz, uid: i64, session_date_and_cost: &SessionDateAndCost) -> Result<(), Custom<String>> {
    // Can always book a zero-cost session even if you already have other bookings.
    if session_date_and_cost.cost == 0 {
        return Ok(());
    }

    // Get the date/time of the session and work out the start and end of the week that the session occurs in
    let datetime_in_local = timezone.from_utc_datetime(&session_date_and_cost.datetime.naive_utc());
    let start_of_week_local = datetime_in_local
        .checked_sub_days(Days::new(datetime_in_local.weekday().num_days_from_monday() as u64)).unwrap()
        .with_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
        .unwrap();
    let end_of_week_local = start_of_week_local
        .checked_add_days(Days::new(7)).unwrap();

    // Find other bookings in the same week (only sessions with nonzero cost)
    let existing_bookings: Vec<MemberExistingBooking> = query_as("SELECT b.person_id AS person_id, b.session_id AS session_id, s.datetime AS datetime, s.cost AS cost \
            FROM booking AS b \
            JOIN session AS s ON b.session_id = s.id \
            WHERE b.person_id = $1 \
            AND s.cost > 0 \
            AND s.datetime >= $2 \
            AND s.datetime < $3")
        .bind(uid)
        .bind(start_of_week_local)
        .bind(end_of_week_local)
        .fetch_all(pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;

    // Error if there is at least one existing booking
    if !existing_bookings.is_empty() {
        return Err(Custom(Status::Forbidden, format!("Cannot book session: member already has {} booking(s) in this week.", existing_bookings.len())));
    }

    Ok(())
}

async fn book_session_no_max_bookings(pool: &PgPool, person_id: i64, session_id: i64, credits_used: i16) -> Result<(), Custom<String>> {
    query_as("INSERT INTO booking (person_id, session_id, credits_used) VALUES ($1, $2, $3) RETURNING person_id, session_id")
        .bind(person_id)
        .bind(session_id)
        .bind(credits_used)
        .fetch_one(pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
}

#[derive(FromRow)]
struct SessionWithMaxBookingCount {
    id: i64,
    max_booking_count: Option<i64>
}


async fn book_session_with_max_bookings(pool: &PgPool, person_id: i64, session_id: i64, max_bookings: i64, credits_used: i16) -> Result<(), Custom<String>> {
    // Atomically update the booking table to insert a new booking if and only if the count of
    // bookings for the referenced session is less than the maximum. Adapted from this StackOverflow
    // answer: https://dba.stackexchange.com/a/167283
    // NB simple string interpolation without prepared statements is safe because the arguments all
    // are numeric.
    let sql = format!("BEGIN; \
        SELECT id FROM session WHERE id = {} FOR NO KEY UPDATE; \
        INSERT INTO booking (person_id, session_id, credits_used) \
        SELECT {}, {}, {} FROM booking \
        WHERE session_id = {} \
        HAVING count(*) < {} \
        ON CONFLICT DO NOTHING \
        RETURNING person_id, session_id; \
        END;", session_id, person_id, session_id, credits_used, session_id, max_bookings);
    info!("Executing raw SQL: {}", &sql);
    let mut result_stream = raw_sql(sql.as_str()).execute_many(pool);

    let _ = take_result_from_stream(&mut result_stream).await?; // result from BEGIN;
    let _ = take_result_from_stream(&mut result_stream).await?; // result from SELECT..FOR UPDATE;
    let insert_result = take_result_from_stream(&mut result_stream).await?; // result from INSERT..RETURNING;
    let _ = take_result_from_stream(&mut result_stream).await?; // result from COMMIT;
    info!("Insert result: {:?}", insert_result);

    if insert_result.rows_affected() == 0 {
        return Err(Custom(Status::Conflict, format!("Session has reached it maximum number of bookings: {}.", max_bookings)));
    }
    Ok(())
}

async fn get_session_date_and_cost(pool: &PgPool, session_id: &i64) -> Result<SessionDateAndCost, Custom<String>> {
    query_as("SELECT id, datetime, cost FROM session WHERE id = $1")
        .bind(&session_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or(Custom(Status::NotFound, format!("no session with id {}", &session_id)))
}

#[delete("/bookings?<session_id>&<person_id>")]
pub async fn delete_booking(state: &State<AppState>, claim: Claims, person_id: i64, session_id: i64) -> Result<Json<SessionBooking>, Custom<String>> {
    _delete_booking(&state.pool, &claim, person_id, session_id).await
}

async fn _delete_booking(pool: &PgPool, claim: &Claims, person_id: i64, session_id: i64) -> Result<Json<SessionBooking>, Custom<String>> {
    if !claim.has_role("admin") {
        if person_id != claim.uid {
            return Err(Custom(Status::Forbidden, "Not allowed to cancel bookings for other users.".to_string()));
        }
        // Error if session is in the past
        if get_session_date_and_cost(pool, &session_id).await?.datetime.lt(&Utc::now()) {
            return Err(Custom(Status::Forbidden, "Cannot cancel past booking.".to_string()));
        }
    }
    let booking_deleted: SessionBooking = query_as("DELETE FROM booking WHERE person_id = $1 AND session_id = $2 RETURNING person_id, session_id, credits_used")
        .bind(person_id)
        .bind(session_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or(Custom(Status::NotFound, format!("No booking found with person_id={} and session_id={}.", person_id, session_id)))?;

    // Restore the credits used for this booking
    if booking_deleted.credits_used.unwrap_or(0) > 0 {
        query_as("UPDATE person SET credits = credits + $1 WHERE id = $2 RETURNING id, credits")
            .bind(booking_deleted.credits_used)
            .bind(person_id)
            .fetch_one(pool)
            .await.map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    }

    Ok(Json(booking_deleted))
}

#[derive(Deserialize)]
pub struct BookingUpdate {
    attended: bool
}

#[put("/bookings?<session_id>&<person_id>", data="<booking_update>")]
pub async fn update_booking(state: &State<AppState>, claim: Claims, person_id: i64, session_id: i64, booking_update: Json<BookingUpdate>) -> Result<NoContent, Custom<String>> {
    claim.assert_roles_contains("admin")?;
    let _ = query_as("UPDATE booking SET attended = $1 WHERE person_id = $2 AND session_id = $3 RETURNING person_id, session_id")
        .bind(booking_update.attended)
        .bind(person_id)
        .bind(session_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or(Custom(Status::NotFound, format!("No booking found with person_id={} and session_id={}.", person_id, session_id)))?;
    Ok(NoContent)
}

#[derive(Serialize, FromRow)]
pub struct AttendanceStat {
    person_id: i64,
    name: String,
    email: String,
    attended_count: i64
}

#[get("/stats/attendance?<from>&<to>&<session_type>")]
pub async fn get_attendance_stats(state: &State<AppState>, claim: Claims, from: Option<String>, to: Option<String>, session_type: Vec<i32>) -> Result<Json<Vec<AttendanceStat>>, Custom<String>> {
    claim.assert_roles_contains("admin")?;
    let mut qb = QueryBuilder::new("\
        SELECT p.id AS person_id, p.name AS name, p.email AS email, ( \
            SELECT COUNT(*) \
            FROM booking \
            JOIN session ON booking.session_id = session.id \
            WHERE booking.person_id = p.id \
            AND booking.attended = TRUE ");

    if let Some(from) = parse_opt_date(from)? {
        qb.push(" AND session.datetime >= ");
        qb.push_bind(from);
    }
    if let Some(to) = parse_opt_date(to)? {
        qb.push(" AND session.datetime <= ");
        qb.push_bind(to);
    }

    if !session_type.is_empty() {
        let session_types_str = session_type.into_iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        qb.push(" AND session.session_type IN (");
        qb.push(session_types_str);
        qb.push(")");
    } else {
        // Cannot write "IN ()" so we create a clause that is always false
        qb.push(" AND FALSE");
    }


    qb.push(") AS attended_count \
        FROM person AS p \
        ORDER BY attended_count DESC, name \
        LIMIT 10");
    info!("fetching: {}", qb.sql());

    let stats = qb.build_query_as()
        .fetch_all(&state.pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;

    Ok(Json(stats))
}

#[cfg(test)]
mod tests {
    use std::ops::Add;
    use chrono::{DateTime, Duration, TimeDelta, Utc};
    use chrono_tz::Tz;
    use rocket::http::Status;
    use rocket::serde::json::Json;
    use rocket::response::status::Custom;
    use sqlx::{Executor, FromRow, PgPool, query_as};
    use crate::bookings::{_delete_booking, _list_bookings, SessionBooking};
    use crate::claims::Claims;
    use crate::{CountResult, UserLoginRecord};

    #[derive(FromRow)]
    struct IntRecord {
        id: i32
    }

    #[derive(FromRow)]
    struct BigintRecord {
        id: i64
    }
    async fn create_person(pool: &PgPool, email: &str, roles: &str, credits: i32) -> i64 {
        let member_id: BigintRecord = query_as("insert into person (name, email, roles, credits) values ('Test User', $1, $2, $3) returning id")
            .bind(email)
            .bind(roles)
            .bind(credits)
            .fetch_one(pool)
            .await.unwrap();
        member_id.id
    }
    async fn create_session(pool: &PgPool, datetime: &DateTime<Utc>, trainer_id: i64, session_type_name: &str, location_name: &str) -> i64 {
        create_session_max_bookings(pool, datetime, trainer_id, session_type_name, location_name, None).await
    }

    async fn create_session_max_bookings(pool: &PgPool, datetime: &DateTime<Utc>, trainer_id: i64, session_type_name: &str, location_name: &str, max_bookings: Option<i64>) -> i64 {
        let session_type_id: IntRecord = query_as("select id from session_type where name = $1")
            .bind(session_type_name)
            .fetch_one(pool).await.unwrap();

        let location_id: IntRecord = query_as("select id from location where name = $1")
            .bind(location_name)
            .fetch_one(pool).await.unwrap();

        let session_id_record: BigintRecord = query_as("insert into session (datetime, duration_mins, session_type, location, trainer, cost, max_booking_count) \
            VALUES ($1, 60, $2, $3, $4, 1, $5) \
            RETURNING id
        ")
            .bind(datetime)
            .bind(session_type_id.id)
            .bind(location_id.id)
            .bind(trainer_id)
            .bind(max_bookings)
            .fetch_one(pool).await.unwrap();

        session_id_record.id
    }
    async fn count_bookings(pool: &PgPool) -> i64 {
        let record: CountResult = query_as("select count(*) from booking")
            .fetch_one(pool)
            .await
            .unwrap();
        record.count
    }

    #[sqlx::test]
    async fn book_session_full_member(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer_id = create_person(&pool, "trainer@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "member@example.org", "member", 0).await;
        let session_id = create_session(&pool, &Utc::now().add(TimeDelta::days(1)), trainer_id, "HIIT", "Oak Hill Park").await;
        let booking = SessionBooking {
            person_id: member_id,
            session_id,
            credits_used: None
        };

        // Precondition: zero bookings
        assert_eq!(0, count_bookings(&pool).await);

        // Create booking
        let timezone: Tz = "Europe/London".parse().unwrap();
        let claim = Claims::create(member_id, "joe@example.com", &Some("011111".to_string()), &vec!["member".to_string()], Duration::minutes(1));
        crate::bookings::_create_booking(&pool, &timezone, &claim, Json(booking)).await.unwrap();

        // Postcondition: 1 booking
        assert_eq!(1, count_bookings(&pool).await);
    }

    #[sqlx::test]
    async fn book_session_non_member(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer_id = create_person(&pool, "trainer@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "member@example.org", "member", 0).await;
        let session_id = create_session(&pool, &Utc::now().add(TimeDelta::days(1)), trainer_id, "HIIT", "Oak Hill Park").await;
        let booking = SessionBooking {
            person_id: member_id,
            session_id,
            credits_used: None
        };

        // Precondition: zero bookings
        assert_eq!(0, count_bookings(&pool).await);

        // Create booking
        let timezone: Tz = "Europe/London".parse().unwrap();
        let claim = Claims::create(member_id, "joe@example.com", &Some("011111".to_string()), &vec![], Duration::minutes(1));
        let result = crate::bookings::_create_booking(&pool, &timezone, &claim, Json(booking)).await;
        assert!(result.is_err());
        assert_eq!(Custom(Status::Forbidden, "Missing or expired membership, and no PAYG credits.".to_string()), result.err().unwrap());

        // Postcondition: still zero bookings
        assert_eq!(0, count_bookings(&pool).await);
    }

    #[sqlx::test]
    async fn book_session_limited_member_existing_session_same_week(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer_id = create_person(&pool, "trainer@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "member@example.org", "limited-member", 0).await;
        let datetime = Utc::now().add(TimeDelta::days(1));
        let session_id_1 = create_session(&pool, &datetime, trainer_id, "HIIT", "Oak Hill Park").await;
        let booking_1 = SessionBooking {
            person_id: member_id,
            session_id: session_id_1,
            credits_used: None
        };
        let session_id_2 = create_session(&pool, &datetime, trainer_id, "On The Move", "Oak Hill Park").await;
        let booking_2 = SessionBooking {
            person_id: member_id,
            session_id: session_id_2,
            credits_used: None
        };
        let timezone: Tz = "Europe/London".parse().unwrap();

        // Precondition: zero bookings
        assert_eq!(0, count_bookings(&pool).await);

        // Create booking 1
        let claim = Claims::create(member_id, "member@example.com", &Some("011111".to_string()), &vec!["limited-member".to_string()], Duration::minutes(1));
        crate::bookings::_create_booking(&pool, &timezone, &claim, Json(booking_1)).await.unwrap();

        // Postcondition 1: one booking
        assert_eq!(1, count_bookings(&pool).await);

        // Create booking 2: fails
        let claim = Claims::create(member_id, "member@example.com", &Some("011111".to_string()), &vec!["limited-member".to_string()], Duration::minutes(1));
        let result = crate::bookings::_create_booking(&pool, &timezone, &claim, Json(booking_2.clone())).await;
        assert!(result.is_err());
        assert_eq!(Custom(Status::Forbidden, "Cannot book session: member already has 1 booking(s) in this week.".to_string()), result.err().unwrap());

        // Postcondition 2: one booking
        assert_eq!(1, count_bookings(&pool).await);

        // Cancel booking 1
        _delete_booking(&pool, &claim, member_id, session_id_1).await.unwrap();

        // Postcondition 3: zero bookings
        assert_eq!(0, count_bookings(&pool).await);

        // Create booking 2: succeeds now
        let claim = Claims::create(member_id, "member@example.com", &Some("011111".to_string()), &vec!["limited-member".to_string()], Duration::minutes(1));
        crate::bookings::_create_booking(&pool, &timezone, &claim, Json(booking_2)).await.unwrap();

        // Postcondition 4: one booking
        assert_eq!(1, count_bookings(&pool).await);
    }

    #[sqlx::test]
    async fn book_session_limited_member_existing_session_next_week(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer_id = create_person(&pool, "trainer@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "member@example.org", "limited-member", 0).await;
        let tomorrow = Utc::now().add(TimeDelta::days(1));
        let next_week = tomorrow.add(TimeDelta::weeks(1));
        let session_id_1 = create_session(&pool, &tomorrow, trainer_id, "HIIT", "Oak Hill Park").await;
        let booking_1 = SessionBooking {
            person_id: member_id,
            session_id: session_id_1,
            credits_used: None
        };
        let session_id_2 = create_session(&pool, &next_week, trainer_id, "On The Move", "Oak Hill Park").await;
        let booking_2 = SessionBooking {
            person_id: member_id,
            session_id: session_id_2,
            credits_used: None
        };
        let timezone: Tz = "Europe/London".parse().unwrap();

        // Precondition: zero bookings
        assert_eq!(0, count_bookings(&pool).await);

        // Create booking 1
        let claim = Claims::create(member_id, "member@example.com", &Some("011111".to_string()), &vec!["limited-member".to_string()], Duration::minutes(1));
        crate::bookings::_create_booking(&pool, &timezone, &claim, Json(booking_1)).await.unwrap();

        // Postcondition 1: one booking
        assert_eq!(1, count_bookings(&pool).await);

        // Create booking 2: succeeds because it's next week
        let claim = Claims::create(member_id, "member@example.com", &Some("011111".to_string()), &vec!["limited-member".to_string()], Duration::minutes(1));
        crate::bookings::_create_booking(&pool, &timezone, &claim, Json(booking_2.clone())).await.unwrap();

        // Postcondition 2: two bookings
        assert_eq!(2, count_bookings(&pool).await);
    }

    #[sqlx::test]
    async fn book_session_non_member_using_credit_not_opted_in(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer_id = create_person(&pool, "trainer@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "member@example.org", "member", 5).await;
        let session_id = create_session(&pool, &Utc::now().add(TimeDelta::days(1)), trainer_id, "HIIT", "Oak Hill Park").await;
        let booking = SessionBooking {
            person_id: member_id,
            session_id,
            credits_used: None
        };

        // Precondition: zero bookings
        assert_eq!(0, count_bookings(&pool).await);

        // Create booking
        let timezone: Tz = "Europe/London".parse().unwrap();
        let claim = Claims::create(member_id, "joe@example.com", &Some("011111".to_string()), &vec![], Duration::minutes(1));
        let result = crate::bookings::_create_booking(&pool, &timezone, &claim, Json(booking)).await;
        assert!(result.is_err());
        assert_eq!(Custom(Status::PaymentRequired, "Opt in to use credits for booking.".to_string()), result.err().unwrap());

        // Postcondition: still zero bookings
        assert_eq!(0, count_bookings(&pool).await);
    }

    #[sqlx::test]
    async fn book_session_non_member_using_credit_opted_in(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer_id = create_person(&pool, "trainer@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "member@example.org", "", 5).await;
        let session_id = create_session(&pool, &Utc::now().add(TimeDelta::days(1)), trainer_id, "HIIT", "Oak Hill Park").await;
        let booking = SessionBooking {
            person_id: member_id,
            session_id,
            credits_used: Some(1)
        };

        // Precondition: zero bookings
        assert_eq!(0, count_bookings(&pool).await);

        // Create booking
        let timezone: Tz = "Europe/London".parse().unwrap();
        let claim = Claims::create(member_id, "joe@example.com", &Some("011111".to_string()), &vec![], Duration::minutes(1));
        let result = crate::bookings::_create_booking(&pool, &timezone, &claim, Json(booking)).await.unwrap();

        // Check that the booking has the used credits
        let created_booking: SessionBooking = query_as("SELECT person_id, session_id, credits_used FROM booking WHERE person_id = $1 AND session_id = $2")
            .bind(member_id)
            .bind(session_id)
            .fetch_one(&pool)
            .await.unwrap();
        assert_eq!(Some(1), created_booking.credits_used);
        let bookings_list = _list_bookings(&pool, &claim, None, Some(member_id), None, None).await.unwrap();
        assert_eq!(1, bookings_list.len());
        assert_eq!(1, bookings_list.get(0).unwrap().credits_used);

        // Check that the user has been debited one credit
        let member_record = UserLoginRecord::load_by_id(&pool, member_id)
            .await.unwrap().unwrap();
        assert_eq!(4, member_record.credits);

        // Cancel booking
        _delete_booking(&pool, &claim, member_id, session_id).await.unwrap();
        // Postcondition: zero bookings
        assert_eq!(0, count_bookings(&pool).await);

        // Check that the user's credit has been restored
        let member_record = UserLoginRecord::load_by_id(&pool, member_id)
            .await.unwrap().unwrap();
        assert_eq!(5, member_record.credits);
    }

    #[sqlx::test]
    async fn book_session_non_member_using_credit_max_bookings_reached(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer_id = create_person(&pool, "trainer@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "member@example.org", "", 5).await;
        let session_id = create_session_max_bookings(&pool, &Utc::now().add(TimeDelta::days(1)), trainer_id, "HIIT", "Oak Hill Park", Some(0)).await;
        let booking = SessionBooking {
            person_id: member_id,
            session_id,
            credits_used: Some(1)
        };

        // Precondition: zero bookings
        assert_eq!(0, count_bookings(&pool).await);

        // Create booking: fail due to max bookings reached
        let timezone: Tz = "Europe/London".parse().unwrap();
        let claim = Claims::create(member_id, "joe@example.com", &Some("011111".to_string()), &vec![], Duration::minutes(1));
        let booking_result = crate::bookings::_create_booking(&pool, &timezone, &claim, Json(booking)).await.err().unwrap();
        assert_eq!(Custom(Status::Conflict, "Session has reached it maximum number of bookings: 0.".to_string()), booking_result);

        // Still zero bookings
        assert_eq!(0, count_bookings(&pool).await);

        // Check that the user has NOT been debited any credits
        let member_record = UserLoginRecord::load_by_id(&pool, member_id)
            .await.unwrap().unwrap();
        assert_eq!(5, member_record.credits);
    }
}

