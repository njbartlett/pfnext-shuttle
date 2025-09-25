use chrono::{DateTime, Datelike, Days, FixedOffset, NaiveTime, TimeZone};
use chrono_tz::Tz;
use rocket::futures::stream::BoxStream;
use rocket::futures::StreamExt;
use rocket::http::Status;
use rocket::response::status::{Created, Custom, NoContent};
use rocket::serde::json::Json;
use rocket::serde::Serialize;
use rocket::{Route, State};
use serde::Deserialize;
use sqlx::postgres::{PgQueryResult, PgRow};
use sqlx::{query, query_as, query_scalar, raw_sql, Error, FromRow, PgPool, Postgres, QueryBuilder, Row};
use unicode_segmentation::UnicodeSegmentation;
use std::fmt::{Display, Formatter};

use crate::common::{parse_opt_date, to_internal_server_err};
use crate::config::{AppEnv, Config};
use crate::notifications::send_email;
use crate::transaction_log::append_log;
use crate::loginsession::{LoginSession, Roles};
use crate::sessions::{SessionLocation, SessionType};

#[cfg(test)]
use crate::mock_chrono::Utc;
#[cfg(not(test))]
use chrono::Utc;

const ROLE_FULL_MEMBER: &str = "member";
const ROLE_TRAINER: &str = "trainer";
const ROLE_LIMITED_MEMBER: &str = "limited-member";

const COMMENT_MAX_LENGTH: usize = 1000;

pub fn routes() -> Vec<Route> {
    routes![
        list_bookings,
        create_booking,
        delete_booking,
        update_booking,
        get_attendance_stats,
        list_waitlist,
        add_waitlist,
        delete_waitlist,
        list_feedback
    ]
}

#[derive(Serialize, Deserialize, FromRow, Debug, Clone, PartialEq)]
struct SessionBooking {
    person_id: i64,
    session_id: i64,
    credits_used: Option<i16>
}

#[derive(Serialize, Debug, PartialEq)]
struct SessionBookingFull {
    person_id: i64,
    person_name: String,
    person_email: String,
    session_id: i64,
    session_datetime: DateTime<FixedOffset>,
    session_duration_mins: i32,
    session_location: Option<SessionLocation>,
    session_type: SessionType,
    attended: bool,
    credits_used: i16,
    booked_timestamp: Option<DateTime<FixedOffset>>
}

impl FromRow<'_, PgRow> for SessionBookingFull {
    fn from_row(row: &'_ PgRow) -> Result<Self, Error> {
        let location_id: Option<i32> = row.try_get("session_location_id").ok();
        let location: Option<SessionLocation> = match location_id {
            Some(id) => Some(SessionLocation{
                id,
                name: row.try_get("session_location_name")?,
                address: row.try_get("session_location_address")?,
                url: row.try_get("session_location_url")?,
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
                cost: row.try_get("session_type_cost")?,
                deprecated: row.try_get("session_type_deprecated")?
            },
            attended: row.try_get("attended").ok().unwrap_or(false),
            credits_used: row.try_get("credits_used").ok().unwrap_or(0),
            booked_timestamp: row.try_get("booked_timestamp")?
        })
    }
}

impl SessionBookingFull {
    fn build_query<'a>(
        session_id: Option<i64>,
        person_id: Option<i64>,
        from: Option<DateTime<FixedOffset>>,
        to: Option<DateTime<FixedOffset>>
    ) -> QueryBuilder<'a, Postgres> {
        // Build the Query
        let mut qb = QueryBuilder::new("SELECT b.person_id, p.name AS person_name, p.email AS person_email, b.session_id, b.credits_used, b.booked_timestamp, \
                    s.datetime AS session_datetime, s.duration_mins AS session_duration_mins, s.location AS session_location_id, l.name AS session_location_name, l.address AS session_location_address, l.url AS session_location_url, \
                    s.session_type AS session_type_id, t.name AS session_type_name, t.requires_trainer AS session_type_requires_trainer, t.cost AS session_type_cost, t.deprecated AS session_type_deprecated, b.attended \
                FROM booking as b \
                JOIN person AS p ON b.person_id = p.id \
                JOIN session AS s ON b.session_id = s.id \
                JOIN session_type AS t ON s.session_type = t.id \
                LEFT JOIN location AS l ON s.location = l.id ");

        let mut where_op = String::from(" WHERE");
        if let Some(person_id) = person_id {
            qb.push(where_op + " b.person_id = ");
            qb.push_bind(person_id);
            where_op = String::from(" AND");
        }

        if let Some(session_id) = session_id {
            qb.push(where_op + " b.session_id = ");
            qb.push_bind(session_id);
            where_op = String::from(" AND");
        }
        if let Some(from) = from {
            qb.push(where_op + " s.datetime >= ");
            qb.push_bind(from);
            where_op = String::from(" AND");
        }
        if let Some(to) = to {
            qb.push(where_op + " s.datetime <= ");
            qb.push_bind(to);
        }

        qb.push(" ORDER BY session_datetime, person_name");

        qb
    }

    async fn list(
        pool: &PgPool,
        session_id: Option<i64>,
        person_id: Option<i64>,
        from: Option<DateTime<FixedOffset>>,
        to: Option<DateTime<FixedOffset>>
    ) -> Result<Vec<SessionBookingFull>, sqlx::Error> {
        Self::build_query(session_id, person_id, from, to)
            .build_query_as()
            .fetch_all(pool)
            .await
    }

    async fn find(
        pool: &PgPool,
        session_id: i64,
        person_id: i64
    ) -> Result<Option<SessionBookingFull>, sqlx::Error> {
        Self::build_query(Some(session_id), Some(person_id), None, None)
            .build_query_as()
            .fetch_optional(pool)
            .await
    }
}

#[get("/bookings?<session_id>&<person_id>&<from>&<to>")]
async fn list_bookings(
    pool: &State<PgPool>,
    login: LoginSession,
    session_id: Option<i64>,
    person_id: Option<i64>,
    from: Option<String>,
    to: Option<String>
) -> Result<Json<Vec<SessionBookingFull>>, Custom<String>> {
    // Permission Check...
    let allowed: bool;
    if login.is_admin() {
        // Admins can see anything
        allowed = true;
    } else if login.has_role(ROLE_TRAINER) {
        // Trainers can their own bookings AND bookings for sessions on which they are the trainer.
        if person_id == Some(login.uid) {
            // Trainer is the owner of the booking
            allowed = true;
        } else {
            if let Some(trained_session_id) = session_id {
                allowed = user_is_admin_for_session(pool, &login, trained_session_id).await?;
            } else {
                allowed = false;
            }
        }
    } else {
        // Everybody else can see only their own bookings.
        allowed = person_id == Some(login.uid);
    }
    if !allowed {
        return Err(Custom(Status::Forbidden, "user is not allowed to view bookings for selected person and/or session".to_string()));
    }

    let bookings = SessionBookingFull::list(
        pool,
        session_id,
        person_id,
        parse_opt_date(from)?,
        parse_opt_date(to)?,
    ).await.map_err(to_internal_server_err)?;

    Ok(Json(bookings))
}

async fn take_result_from_stream<'a>(stream: &mut BoxStream<'a, Result<PgQueryResult, Error>>) -> Result<PgQueryResult, Custom<String>> {
    stream.next()
        .await
        .ok_or(Custom(Status::InternalServerError, "no more results".to_string()))?
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
}

type CreditsCost = i16;

#[post("/bookings", data="<booking>")]
async fn create_booking(
    pool: &State<PgPool>,
    config: &State<Config>,
    login: LoginSession,
    booking: Json<SessionBooking>
) -> Result<Created<Json<SessionBooking>>, Custom<String>> {
    // Load details of the booking person and session
    let person_and_session = PersonSessionBookingDetails::load(pool, booking.person_id, booking.session_id).await?;

    // Admins can always make a booking for any user. Admin for a session includes the trainer of that session.
    let credits_to_use = if user_is_admin_for_session(pool, &login, booking.session_id).await? {
        0
    } else {
        // Non-admins can only book on their own behalf
        if login.uid != booking.person_id {
            info!("person id {} attempted to book session on behalf of person id {}; denied: missing admin role", login.uid, booking.person_id);
            return Err(Custom(Status::Forbidden, "Cannot create a booking for another user!".to_string()));
        }

        // Non-admins can only book future sessions
        if person_and_session.datetime.lt(&Utc::now()) {
            info!("person id {} attempted to book session in past (session id {}, date {}); denied: missing admin role", login.uid, booking.session_id, person_and_session.datetime);
            return Err(Custom(Status::Forbidden, "Cannot create booking in the past!".to_string()));
        }

        let credits_to_use = booking.credits_used.unwrap_or(0);
        check_bookable(pool, config, &person_and_session, credits_to_use >= person_and_session.cost).await?
    };

    make_booking(pool, &person_and_session, credits_to_use, login.email)
        .await
        .map(|_| Created::new(format!("/bookings?sessionid={},person_id={}", booking.session_id, booking.person_id)))

}

async fn check_bookable(
    pool: &PgPool,
    config: &Config,
    person_and_session: &PersonSessionBookingDetails,
    use_credits: bool
) -> Result<CreditsCost, Custom<String>> {
    // Check whether the user has full membership or a usable limited membership
    let membership_check: Result<CreditsCost, Custom<String>>;
   
    if person_and_session.person_roles.has_role(ROLE_FULL_MEMBER) {
        membership_check = Ok(0);
    } else if person_and_session.person_roles.has_role(ROLE_LIMITED_MEMBER) {
        // Can always book a zero-cost session even if you already have other bookings.
        membership_check = if person_and_session.cost == 0 {
            Ok(0)
        } else {
            let timezone = config.get_timezone().map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
            check_limited_member_has_no_bookings_in_same_week(pool, &timezone, person_and_session.person_id, &person_and_session.datetime)
                .await
                .map(|_| 0)
        }
    } else {
        membership_check = Err(Custom(Status::Forbidden, "Missing or expired membership, and insufficient Pay As You Go credits.".to_string()));
    }

    // If no usable membership, check for credits
    if membership_check.is_err() && membership_check.as_ref().err().unwrap().0 == Status::Forbidden {
        // let user_record = UserLoginRecord::load_by_id(pool, person_id).await
        //     .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        //     .ok_or(Custom(Status::Unauthorized, "missing user record".to_string()))?;

        if person_and_session.person_credits >= person_and_session.cost {
            if !use_credits {
                Err(Custom(Status::PaymentRequired, "Opt in to use credits for booking.".to_string()))
            } else {
                Ok(person_and_session.cost)
            }
        } else {
            membership_check
        }
    } else {
        // Technical errors other than forbidden should break out
        membership_check
    }
}

async fn make_booking(
    pool: &PgPool,
    person_and_session: &PersonSessionBookingDetails,
    credits_to_use: CreditsCost,
    originator: String
) -> Result<(), Custom<String>> {
    // Make the booking
    if let Some(max_booking_count) = person_and_session.max_booking_count {
        book_session_with_max_bookings(pool, person_and_session.person_id, person_and_session.session_id, max_booking_count, credits_to_use).await?;
    } else {
        book_session_no_max_bookings(pool, person_and_session.person_id, person_and_session.session_id, credits_to_use).await?;
    }

    // Debit the credits used from the user if required
    if credits_to_use > 0 {
        query("UPDATE person SET credits = credits - $1 WHERE id = $2 RETURNING id")
            .bind(credits_to_use)
            .bind(person_and_session.person_id)
            .fetch_one(pool)
            .await.map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    }

    append_log(pool, &Some(originator), "CREATED BOOKING", format!("{}, credits_used={}", person_and_session, credits_to_use).as_str()).await?;
    Ok(())
}

async fn user_is_admin_for_session(pool: &PgPool, login: &LoginSession, session_id: i64) -> Result<bool, Custom<String>> {
    if login.is_admin() {
        return Ok(true);
    }
    if login.has_role(ROLE_TRAINER) {
        // Need to read the trainer ID of the selected session
        let session_trainer_id: i64 = query("SELECT trainer AS id FROM session WHERE id = $1")
            .bind(session_id)
            .fetch_one(pool)
            .await
            .and_then(|r| r.try_get("id"))
            .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
        return Ok(session_trainer_id == login.uid);
    }
    return Ok(false);
}

#[derive(FromRow)]
struct PersonSessionBookingDetails {
    person_id: i64,
    person_name: String,
    person_email: String,
    person_roles: Roles,
    person_credits: CreditsCost,
    datetime: DateTime<FixedOffset>,
    cost: i16,
    session_id: i64,
    session_type_name: String,
    session_location_name: Option<String>,
    booking_count: i64,
    max_booking_count: Option<i64>,
}

impl Display for PersonSessionBookingDetails {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "person={}, type={}, datetime={}, location={}, cost={}",
            self.person_name,
            self.session_type_name,
            self.datetime,
            self.session_location_name.as_ref().unwrap_or(&"None".to_string()),
            self.cost))
    }
}

impl PersonSessionBookingDetails {
    async fn load(pool: &PgPool, person_id: i64, session_id: i64) -> Result<PersonSessionBookingDetails, Custom<String>> {
        query_as("SELECT p.id AS person_id, p.name AS person_name, p.email AS person_email, p.roles AS person_roles, p.credits AS person_credits, s.id AS session_id, s.datetime, s.cost, st.name AS session_type_name, l.name AS session_location_name, s.max_booking_count,
            (SELECT COUNT(*) FROM booking AS b WHERE b.session_id = $1) AS booking_count
            FROM person AS p, session as s
            JOIN session_type AS st ON s.session_type = st.id
            LEFT JOIN location AS l ON s.location = l.id
            WHERE p.id = $2
            AND s.id = $3")
            .bind(session_id)
            .bind(person_id)
            .bind(session_id)
            .fetch_one(pool)
            .await
            .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
    }
}

async fn check_limited_member_has_no_bookings_in_same_week(pool: &PgPool, timezone: &Tz, uid: i64, session_datetime: &DateTime<FixedOffset>) -> Result<(), Custom<String>> {
    // Get the date/time of the session and work out the start and end of the week that the session occurs in
    let datetime_in_local = timezone.from_utc_datetime(&session_datetime.naive_utc());
    let start_of_week_local = datetime_in_local
        .checked_sub_days(Days::new(datetime_in_local.weekday().num_days_from_monday() as u64)).unwrap()
        .with_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
        .unwrap();
    let end_of_week_local = start_of_week_local
        .checked_add_days(Days::new(7)).unwrap();

    // Find other bookings in the same week (only sessions with nonzero cost)
    let existing_bookings_count: i64 = query("SELECT COUNT(*)
            FROM booking AS b JOIN session AS s ON b.session_id = s.id
            WHERE b.person_id = $1
            AND s.cost > 0
            AND s.datetime >= $2
            AND s.datetime < $3")
        .bind(uid)
        .bind(start_of_week_local)
        .bind(end_of_week_local)
        .fetch_one(pool)
        .await
        .and_then(|r| r.try_get(0))
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;

    // Error if there is at least one existing booking
    if existing_bookings_count > 0 {
        return Err(Custom(Status::Forbidden, format!("Cannot book session: member already has {} booking(s) in this week.", existing_bookings_count)));
    }

    Ok(())
}

async fn book_session_no_max_bookings(pool: &PgPool, person_id: i64, session_id: i64, credits_used: CreditsCost) -> Result<(), Custom<String>> {
    let now = Utc::now();
    query_as("INSERT INTO booking (person_id, session_id, credits_used, booked_timestamp) VALUES ($1, $2, $3, $4) RETURNING person_id, session_id")
        .bind(person_id)
        .bind(session_id)
        .bind(credits_used)
        .bind(now)
        .fetch_one(pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
}

async fn book_session_with_max_bookings(pool: &PgPool, person_id: i64, session_id: i64, max_bookings: i64, credits_used: CreditsCost) -> Result<(), Custom<String>> {
    let time_now_iso8601 = Utc::now().to_rfc3339();
    // Atomically update the booking table to insert a new booking if and only if the count of
    // bookings for the referenced session is less than the maximum. Adapted from this StackOverflow
    // answer: https://dba.stackexchange.com/a/167283
    //
    // NB simple string interpolation without prepared statements is safe for numeric arguments and for the generated time string
    let sql = format!("BEGIN;
        SELECT id FROM session WHERE id = {session_id} FOR NO KEY UPDATE;
        INSERT INTO booking (person_id, session_id, credits_used, booked_timestamp)
            SELECT {person_id}, {session_id}, {credits_used}, '{time_now_iso8601}'
            FROM booking
            WHERE session_id = {session_id}
            HAVING count(*) < {max_bookings}
        ON CONFLICT DO NOTHING
        RETURNING person_id, session_id;
        COMMIT; END;
    ");
    debug!("Executing raw SQL: {}", &sql);
    let mut result_stream = raw_sql(&sql).execute_many(pool);

    let _ = take_result_from_stream(&mut result_stream).await?; // result from BEGIN;
    let _ = take_result_from_stream(&mut result_stream).await?; // result from SELECT..FOR UPDATE;
    let insert_result = take_result_from_stream(&mut result_stream).await?; // result from INSERT..RETURNING;
    info!("Insert result: {:?}", insert_result);

    if insert_result.rows_affected() == 0 {
        return Err(Custom(Status::Conflict, format!("Session has reached it maximum number of bookings: {}.", max_bookings)));
    }
    Ok(())
}

#[delete("/bookings?<session_id>&<person_id>")]
async fn delete_booking(
    pool: &State<PgPool>,
    config: &State<Config>,
    app_env: &State<AppEnv>,
    login: LoginSession,
    person_id: i64,
    session_id: i64
) -> Result<Json<SessionBooking>, Custom<String>> {
    let is_admin = user_is_admin_for_session(pool, &login, session_id).await?;
    if !is_admin && person_id != login.uid {
        return Err(Custom(Status::Forbidden, "Not allowed to cancel bookings for other users.".to_string()));
    }
    let person_booking_details = PersonSessionBookingDetails::load(pool, person_id, session_id).await?;
    let was_fully_booked = Some(person_booking_details.booking_count) == person_booking_details.max_booking_count;

    // Error if session is in the past
    if !is_admin && person_booking_details.datetime.lt(&Utc::now()) {
        return Err(Custom(Status::Forbidden, "Cannot cancel past booking.".to_string()));
    }
    let booking_deleted: SessionBooking = query_as("DELETE FROM booking WHERE person_id = $1 AND session_id = $2 RETURNING person_id, session_id, credits_used")
        .bind(person_id)
        .bind(session_id)
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or(Custom(Status::NotFound, format!("No booking found with person_id={} and session_id={}.", person_id, session_id)))?;

    // Restore the credits used for this booking
    let credits_refund = booking_deleted.credits_used.unwrap_or(0);
    if credits_refund > 0 {
        query("UPDATE person SET credits = credits + $1 WHERE id = $2 RETURNING id")
            .bind(booking_deleted.credits_used)
            .bind(person_id)
            .fetch_one(pool.inner())
            .await.map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    }

    let log_record = PersonSessionBookingDetails::load(pool, booking_deleted.person_id, booking_deleted.session_id).await?;
    append_log(pool, &Some(login.email), "DELETED BOOKING", format!("{}, credits_refunded={}", log_record, credits_refund).as_str()).await?;

    // Check for waitlist entries that can be promoted to bookings
    if was_fully_booked {
        let _ = promote_from_waitlist(pool, config, app_env, session_id).await
            .inspect_err(|e| error!("Failed to promote waitlisted booking(s): {}/{}", e.0, e.1));
    };
    
    Ok(Json(booking_deleted))
}

async fn promote_from_waitlist(
    pool: &PgPool,
    config: &Config,
    app_env: &AppEnv,
    session_id: i64
) -> Result<(), Custom<String>> {
    let timezone = config.get_timezone().map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;

    info!("Looking for a waitlist entry to promote to booking for session id {session_id}");
    let mut waitlist_entry_opt = WaitlistEntryFull::take_top(pool, session_id).await.map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    while waitlist_entry_opt.is_some() {
        let waitlist_entry = waitlist_entry_opt.unwrap();
        info!("Taken waitlist entry: {:?}", waitlist_entry);
        let waitlisted_person_session = PersonSessionBookingDetails::load(pool, waitlist_entry.person_id, waitlist_entry.session_id).await?;
        let local_datetime = waitlisted_person_session.datetime.with_timezone(&timezone);

        // Check if we can make a booking for the top person on the waitlist
        let check_result = check_bookable(pool, config, &waitlisted_person_session, true).await;
        if check_result.is_ok() {
            // Top waitlist person is eligible to book, so book them.
            // If internal errors occur here they abort the function.
            info!("Waitlist entry IS ELIGIBLE for booking: {:?}", waitlist_entry);
            make_booking(pool, &waitlisted_person_session, check_result.unwrap(), "<< waitlist >>".to_string()).await?;
            let text = format!(include_str!("waitlist_promoted_email.txt"),
                &waitlisted_person_session.person_name,
                &waitlisted_person_session.session_type_name,
                &waitlisted_person_session.session_location_name.as_ref().unwrap_or(&"N/A".to_string()),
                local_datetime.format("%A %-d %B %Y"),
                local_datetime.format("%H:%M"),
            );
            let _ = send_email(config, app_env,
                    &waitlisted_person_session.person_name, &waitlisted_person_session.person_email,
                    "The wait is over, your session booking has been confirmed!",
                    &text);
            return Ok(())
        } else {
            let error = check_result.unwrap_err();
            info!("Waitlist entry IS NOT ELIGIBLE for booking: {:?}: {:?}", waitlist_entry, error);

            let text = format!(include_str!("waitlist_failed_email.txt"),
                &waitlisted_person_session.person_name,
                &waitlisted_person_session.session_type_name,
                &waitlisted_person_session.session_location_name.as_ref().unwrap_or(&"N/A".to_string()),
                local_datetime.format("%A %-d %B %Y"),
                local_datetime.format("%H:%M"),
                error.1
            );
            let _ = send_email(config, app_env,
                    &waitlisted_person_session.person_name, &waitlisted_person_session.person_email,
                    "We were unable to confirm your booking.",
                    &text);
        }

        // Top item on waitlist was not eligible, take the next one
        info!("Looking for next waitlist entry to promote to booking for session id {session_id}");
        waitlist_entry_opt = WaitlistEntryFull::take_top(pool, session_id).await.map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    }
    info!("No more waitlist entries for session id {session_id}");

    Ok(())
}


#[derive(Deserialize, FromRow, Serialize)]
struct Feedback {
    rating: i16,
    comment: Option<String>
}

impl Feedback {
    async fn query_by_session_id(pool: &PgPool, session_id: i64) -> Result<Vec<Self>, sqlx::Error> {
        query_as("SELECT rating, comment FROM booking WHERE session_id = $1 AND rating IS NOT NULL")
            .bind(session_id)
            .fetch_all(pool)
            .await
    }
}

#[get("/feedback?<session_id>")]
async fn list_feedback(
    pool: &State<PgPool>,
    login: LoginSession,
    session_id: i64
) -> Result<Json<Vec<Feedback>>, Custom<String>> {
    if !user_is_admin_for_session(pool, &login, session_id).await? {
        return Err(Custom(Status::Forbidden, "feedback listing requires session trainer or admin".to_string()));
    }
    Ok(Json(Feedback::query_by_session_id(pool, session_id).await.map_err(to_internal_server_err)?))
}

#[derive(Deserialize)]
struct BookingUpdate {
    attended: Option<bool>,
    feedback: Option<Feedback>
}

#[patch("/bookings?<session_id>&<person_id>", data="<booking_update>")]
async fn update_booking(
    pool: &State<PgPool>,
    login: LoginSession,
    person_id: i64, session_id: i64,
    booking_update: Json<BookingUpdate>
) -> Result<NoContent, Custom<String>> {
    if let Some(attended) = booking_update.attended {
        update_attendance(pool, &login, person_id, session_id, attended).await?;
        if !attended {
            delete_rating(pool, person_id, session_id).await?;
        }
    }

    if let Some(feedback) = &booking_update.feedback {
        update_feedback(pool, &login, person_id, session_id, feedback).await?;
    }

    Ok(NoContent)
}

async fn update_attendance(
    pool: &PgPool,
    login: &LoginSession,
    person_id: i64, session_id: i64,
    attended: bool
) -> Result<i64, Custom<String>> {
    if !user_is_admin_for_session(pool, &login, session_id).await? {
        return Err(Custom(Status::Forbidden, "cannot update booking: must be the session trainer or an admin".to_string()));
    }
    
    query_scalar("UPDATE booking SET attended = $1 WHERE person_id = $2 AND session_id = $3 RETURNING person_id")
        .bind(attended)
        .bind(person_id)
        .bind(session_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?
        .ok_or(Custom(Status::NotFound, format!("No booking found with person_id={} and session_id={}.", person_id, session_id)))
}

async fn update_feedback(
    pool: &PgPool,
    login: &LoginSession,
    person_id: i64, session_id: i64,
    feedback: &Feedback
) -> Result<i64, Custom<String>> {
    if login.uid != person_id {
        return Err(Custom(Status::Forbidden, "invalid user".to_string()));
    }

    let comment_length = feedback.comment.as_ref()
        .map(|s| s.graphemes(true).count())
        .unwrap_or(0);
    if comment_length > COMMENT_MAX_LENGTH {
        return Err(Custom(Status::UnprocessableEntity, format!("comment length {comment_length} exceeds maximum length of {COMMENT_MAX_LENGTH} characters")));
    }

    let session_booking = SessionBookingFull::find(pool, session_id, person_id)
        .await
        .map_err(to_internal_server_err)?
        .ok_or_else(|| Custom(Status::NotFound, "not found".to_string()))?;
    
    if !session_booking.attended {
        return Err(Custom(Status::Forbidden, "cannot rate a session that was not attended".to_string()));
    }

    query_scalar("UPDATE booking SET rating = $1, comment = $2 WHERE person_id = $3 AND session_id = $4 RETURNING person_id")
        .bind(&feedback.rating)
        .bind(&feedback.comment)
        .bind(person_id)
        .bind(session_id)
        .fetch_optional(pool)
        .await
        .map_err(to_internal_server_err)?
        .ok_or(Custom(Status::NotFound, format!("No booking found with person_id={} and session_id={}.", person_id, session_id)))
}

async fn delete_rating(
    pool: &PgPool,
    person_id: i64,
    session_id: i64
) -> Result<i64, Custom<String>> {
    query_scalar("UPDATE booking SET rating = NULL WHERE person_id = $1 AND session_id = $2 RETURNING person_id")
        .bind(person_id)
        .bind(session_id)
        .fetch_optional(pool)
        .await
        .map_err(to_internal_server_err)?
        .ok_or(Custom(Status::NotFound, format!("No boollking found with person_id={} and session_id={}.", person_id, session_id)))
}

#[derive(Serialize, FromRow)]
struct AttendanceStat {
    person_id: i64,
    name: String,
    email: String,
    attended_count: i64
}

#[get("/stats/attendance?<from>&<to>&<session_type>")]
async fn get_attendance_stats(
    pool: &State<PgPool>,
    login: LoginSession,
    from: Option<String>, to: Option<String>, session_type: Vec<i32>
) -> Result<Json<Vec<AttendanceStat>>, Custom<String>> {
    if !login.is_admin() {
        return Err(Custom(Status::Forbidden, "admin role required to view attendance stats".to_string()));
    }
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
        ORDER BY attended_count DESC, name");
    info!("fetching: {}", qb.sql());

    let stats = qb.build_query_as()
        .fetch_all(pool.inner())
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;

    Ok(Json(stats))
}

#[derive(Serialize, Deserialize, Debug)]
struct WaitlistEntryFull {
    id: i64,
    rank: i64,
    person_id: i64,
    person_name: String,
    person_email: String,
    session_id: i64
}

impl WaitlistEntryFull {
    const BASE_QUERY: &str = "SELECT ROW_NUMBER() OVER (ORDER BY w.id ASC) AS waitlist_rank, w.id, w.person_id, p.name AS person_name, p.email AS person_email, w.session_id
        FROM waitlist w
        JOIN person p ON w.person_id = p.id
        WHERE w.session_id = $1
        ORDER BY w.id ASC";
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        Ok(WaitlistEntryFull {
            id: row.try_get("id")?,
            rank: row.try_get("waitlist_rank")?,
            person_id: row.try_get("person_id")?,
            person_name: row.try_get("person_name")?,
            person_email: row.try_get("person_email")?,
            session_id: row.try_get("session_id")?
        })
    }
    async fn list_by_session(pool: &PgPool, session_id: i64) -> Result<Vec<Self>, sqlx::Error> {
        let rows = query(Self::BASE_QUERY)
            .bind(session_id)
            .fetch_all(pool)
            .await?;

        rows.iter().map(Self::from_row).collect()
    }
    async fn get_by_session_and_id(pool: &PgPool, session_id: i64, waitlist_id: i64) -> Result<Self, sqlx::Error> {
        let mut query_str = "SELECT * FROM (".to_string();
        query_str.push_str(Self::BASE_QUERY);
        query_str.push_str(") AS sub WHERE sub.id = $2");
        let row = query(&query_str)
            .bind(session_id)
            .bind(waitlist_id)
            .fetch_one(pool)
            .await?;
        Self::from_row(&row)
    }
    async fn take_top(pool: &PgPool, session_id: i64) -> Result<Option<Self>, sqlx::Error> {
        match query("DELETE FROM waitlist w
            USING person p
            WHERE w.person_id = p.id
            AND w.id = (SELECT id FROM waitlist WHERE session_id = $1 ORDER BY id ASC LIMIT 1)
            RETURNING w.*, 0::INT8 AS waitlist_rank, p.name AS person_name, p.email AS person_email")
            .bind(session_id)
            .fetch_optional(pool)
            .await? {
                Some(row) => Ok(Some(Self::from_row(&row)?)),
                None => Ok(None),
            }
    }
}

#[get("/waitlist?<session_id>")]
async fn list_waitlist(
    pool: &State<PgPool>,
    login: LoginSession,
    session_id: i64
) -> Result<Json<Vec<WaitlistEntryFull>>, Custom<String>> {
    if !login.is_admin() {
        return Err(Custom(Status::Forbidden, "admin role required".to_string()));
    }
    let waitlist = WaitlistEntryFull::list_by_session(pool, session_id).await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    Ok(Json(waitlist))
}

#[derive(Serialize, Deserialize)]
struct WaitlistEntry {
    person_id: i64,
    session_id: i64
}

#[post("/waitlist", data = "<new_entry>")]
async fn add_waitlist(
    pool: &State<PgPool>,
    login: LoginSession,
    new_entry: Json<WaitlistEntry>
) -> Result<Created<Json<WaitlistEntryFull>>, Custom<String>> {
    if !login.is_admin() && login.uid != new_entry.person_id {
        return Err(Custom(Status::Forbidden, "admin role required".to_string()));
    }
    let waitlist_id: i64 = query("INSERT INTO waitlist (person_id, session_id) VALUES ($1, $2) RETURNING id")
        .bind(new_entry.person_id)
        .bind(new_entry.session_id)
        .fetch_one(pool.inner())
        .await
        .and_then(|row| row.try_get("id"))
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;

    let entry = WaitlistEntryFull::get_by_session_and_id(pool, new_entry.session_id, waitlist_id)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;

   
    Ok(Created::new(format!("/waitlist/{waitlist_id}")).body(Json(entry)))
}

#[delete("/waitlist?<session_id>&<person_id>")]
async fn delete_waitlist(
    pool: &State<PgPool>,
    login: LoginSession,
    person_id: i64,
    session_id: i64
) -> Result<NoContent, Custom<String>> {
    if !login.is_admin() && login.uid != person_id {
        return Err(Custom(Status::Forbidden, "admin role required".to_string()));
    }
    query("DELETE FROM waitlist WHERE person_id = $1 AND session_id = $2 RETURNING id")
        .bind(person_id)
        .bind(session_id)
        .fetch_one(pool.inner())
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    Ok(NoContent)
}

#[post("/waitlist/promote", data = "<waitlist_entry>")]
async fn _promote_waitlist(
    pool: &State<PgPool>,
    login: LoginSession,
    waitlist_entry: Json<WaitlistEntry>
) -> Result<NoContent, Custom<String>> {
    if !login.is_admin() && login.uid != waitlist_entry.person_id {
        return Err(Custom(Status::Forbidden, "admin role required".to_string()));
    }
    let result = query("
            WITH moved_entry AS (
                DELETE FROM waitlist WHERE person_id = $1 AND session_id = $2
                RETURNING id AS waitlist_id, person_id, session_id
            )
            INSERT INTO booking (person_id, session_id)
            SELECT person_id, session_id FROM moved_entry")
        .bind(waitlist_entry.person_id)
        .bind(waitlist_entry.session_id)
        .execute(pool.inner())
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    if result.rows_affected() == 0 {
        return Err(Custom(Status::NotFound, format!("no waitlist record found for persion_id={}, session_id={}", waitlist_entry.person_id, waitlist_entry.session_id)));
    }
    Ok(NoContent)
}

#[cfg(test)]
mod tests {
    use std::ops::Add;
    use chrono::{DateTime, Days, FixedOffset, TimeDelta};
    use rocket::http::Status;
    use rocket::serde::json::Json;
    use rocket::response::status::Custom;
    use rocket::State;
    use sqlx::{query, query_as, query_scalar, Executor, FromRow, PgPool, Postgres, Row};
    use crate::loginsession::{LoginSession, Roles};
    use crate::mock_chrono::{set_timestamp_datetime, set_timestamp_rfc3339, Utc};
    use crate::notifications;
    use crate::users::UserLoginRecord;
    use crate::bookings::{add_waitlist, delete_booking, list_bookings, list_waitlist, BookingUpdate, Feedback, SessionBooking, WaitlistEntry};
    use crate::config::{AppEnv, Config};

    #[derive(FromRow)]
    struct IntRecord {
        id: i32
    }

    #[derive(FromRow)]
    struct BigintRecord {
        id: i64
    }

    async fn create_person(pool: &PgPool, name: &str, email: &str, roles: &str, credits: i32) -> i64 {
        let member_id: BigintRecord = query_as("insert into person (name, email, roles, credits) values ($1, $2, $3, $4) returning id")
            .bind(name)
            .bind(email)
            .bind(roles)
            .bind(credits)
            .fetch_one(pool)
            .await.unwrap();
        member_id.id
    }

    async fn create_session(pool: &PgPool, datetime: &DateTime<FixedOffset>, trainer_id: i64, session_type_name: &str, location_name: &str) -> i64 {
        create_session_max_bookings(pool, datetime, trainer_id, session_type_name, location_name, None).await
    }

    async fn create_session_max_bookings(pool: &PgPool, datetime: &DateTime<FixedOffset>, trainer_id: i64, session_type_name: &str, location_name: &str, max_bookings: Option<i64>) -> i64 {
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

    async fn create_booking(pool: &PgPool, member_id: i64, session_id: i64, credits_used: Option<i16>) {
        let timestamp = Utc::now().fixed_offset();
        let _session_id_record: BigintRecord = query_as("insert into booking (person_id, session_id, credits_used, booked_timestamp) values ($1, $2, $3, $4) returning session_id as id")
            .bind(member_id)
            .bind(session_id)
            .bind(credits_used)
            .bind(timestamp)
            .fetch_one(pool).await.unwrap();
    }

    async fn count_bookings(pool: &PgPool) -> i64 {
        query("SELECT COUNT(*) FROM booking")
            .fetch_one(pool)
            .await
            .and_then(|r| r.try_get(0))
            .unwrap()
    }

    async fn count_bookings_attended(pool: &PgPool, attended: bool) -> i64 {
        query("SELECT COUNT(*) FROM booking WHERE attended = $1")
            .bind(attended)
            .fetch_one(pool)
            .await
            .and_then(|r| r.try_get(0))
            .unwrap()
    }

    async fn read_logged(pool: &PgPool) -> Vec<(String, String, String)> {
        query_as("SELECT person, type, detail FROM eventlog")
            .fetch_all(pool)
            .await
            .unwrap()
    }

    #[sqlx::test]
    async fn book_session_admin(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        set_timestamp_rfc3339("2025-01-01T00:00:00Z");
        let session_time = DateTime::parse_from_rfc3339("2025-01-01T08:00:00Z").unwrap();

        let admin_id = create_person(&pool, "Admin User", "admin@example.org", "admin", 0).await;
        let trainer_id = create_person(&pool, "Trainer User", "trainer@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "Member User", "member@example.org", "member", 0).await;
        let session_id = create_session(&pool, &session_time, trainer_id, "HIIT", "Oak Hill Park").await;
        let booking = crate::bookings::SessionBooking {
            person_id: member_id,
            session_id,
            credits_used: None
        };

        // Create booking
        
        let login = create_login(admin_id, "Admin User", "admin");
        crate::bookings::create_booking(State::from(&pool), State::from(&Config::load().unwrap()), login, Json(booking)).await.unwrap();
        assert_eq!(1, count_bookings(&pool).await);

        assert_eq!(vec![("Admin User@example.org".to_string(), "CREATED BOOKING".to_string(), format!("person=Member User, type=HIIT, datetime={}, location=Oak Hill Park, cost=1, credits_used=0", session_time))], read_logged(&pool).await);
    }

    #[sqlx::test]
    async fn cancel_booking_admin(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let session_time = Utc::now().fixed_offset().add(TimeDelta::days(1));
        let admin_id = create_person(&pool, "Admin User", "admin@example.org", "admin", 0).await;
        let trainer_id = create_person(&pool, "Test User", "trainer@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "Member User", "member@example.org", "member", 0).await;
        let session_id = create_session(&pool, &session_time, trainer_id, "HIIT", "Oak Hill Park").await;
        create_booking(&pool, member_id, session_id, None).await;
        assert_eq!(1, count_bookings(&pool).await);

        let login = create_login(admin_id, "admin", "admin");
        crate::bookings::delete_booking(State::from(&pool), State::from(&Config::load().unwrap()), State::from(&AppEnv::default()), login, member_id, session_id).await.unwrap();
        assert_eq!(0, count_bookings(&pool).await);

        assert_eq!(vec![("admin@example.org".to_string(), "DELETED BOOKING".to_string(), format!("person=Member User, type=HIIT, datetime={}, location=Oak Hill Park, cost=1, credits_refunded=0", session_time))], read_logged(&pool).await);
    }

    #[sqlx::test]
    async fn book_session_non_admin_other_member(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer_id = create_person(&pool, "Test User", "trainer@example.org", "member,trainer", 0).await;
        let member1_id = create_person(&pool, "Test User", "member1@example.org", "member", 0).await;
        let member2_id = create_person(&pool, "Test User", "member2@example.org", "member", 0).await;
        let session_id = create_session(&pool, &Utc::now().fixed_offset().add(TimeDelta::days(1)), trainer_id, "HIIT", "Oak Hill Park").await;
        let booking = crate::bookings::SessionBooking {
            person_id: member1_id,
            session_id,
            credits_used: None
        };

        // Create booking
        let login = create_login(member2_id, "member", "member");
        let result = crate::bookings::create_booking(State::from(&pool), State::from(&Config::load().unwrap()), login, Json(booking)).await;
        assert_eq!(Err(Custom(Status::Forbidden, "Cannot create a booking for another user!".to_string())), result);

        assert_eq!(0, read_logged(&pool).await.len());
    }

    #[sqlx::test]
    async fn cancel_booking_non_admin_same_member(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer_id = create_person(&pool, "Trainer User", "trainer@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "Member User", "member@example.org", "member", 0).await;
        let session_time = Utc::now().fixed_offset().add(TimeDelta::days(1));
        let session_id = create_session(&pool, &session_time, trainer_id, "HIIT", "Oak Hill Park").await;
        create_booking(&pool, member_id, session_id, None).await;
        assert_eq!(1, count_bookings(&pool).await);

        let login = create_login(member_id, "member", "member");
        crate::bookings::delete_booking(State::from(&pool), State::from(&Config::load().unwrap()), State::from(&AppEnv::default()), login, member_id, session_id).await.unwrap();
        assert_eq!(0, count_bookings(&pool).await);

        assert_eq!(vec![("member@example.org".to_string(), "DELETED BOOKING".to_string(), format!("person=Member User, type=HIIT, datetime={}, location=Oak Hill Park, cost=1, credits_refunded=0", session_time))], read_logged(&pool).await);
    }

    #[sqlx::test]
    async fn cancel_booking_non_admin_other_member(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer_id = create_person(&pool, "Test User", "trainer@example.org", "member,trainer", 0).await;
        let member1_id = create_person(&pool, "Test User", "member1@example.org", "member", 0).await;
        let member2_id = create_person(&pool, "Test User", "member2@example.org", "member", 0).await;
        let session_id = create_session(&pool, &Utc::now().fixed_offset().add(TimeDelta::days(1)), trainer_id, "HIIT", "Oak Hill Park").await;
        create_booking(&pool, member1_id, session_id, None).await;
        assert_eq!(1, count_bookings(&pool).await);

        let login = create_login(member2_id, "member", "member");
        let result = crate::bookings::delete_booking(State::from(&pool), State::from(&Config::load().unwrap()), State::from(&AppEnv::default()), login, member1_id, session_id).await;
        assert_eq!(Err(Custom(Status::Forbidden, "Not allowed to cancel bookings for other users.".to_string())), result);

        assert_eq!(0, read_logged(&pool).await.len());
    }

    #[sqlx::test]
    async fn book_session_non_admin_trainer_of_session(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer_id = create_person(&pool, "Trainer User", "trainer@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "Member User", "member1@example.org", "member", 0).await;
        let session_time = Utc::now().fixed_offset().add(TimeDelta::days(1));
        let session_id = create_session(&pool, &session_time, trainer_id, "HIIT", "Oak Hill Park").await;
        let booking = crate::bookings::SessionBooking {
            person_id: member_id,
            session_id,
            credits_used: None
        };

        // Create booking
        let login = create_login(trainer_id, "trainer", "trainer");
        crate::bookings::create_booking(State::from(&pool), State::from(&Config::load().unwrap()), login, Json(booking)).await.unwrap();
        assert_eq!(1, count_bookings(&pool).await);

        assert_eq!(vec![("trainer@example.org".to_string(), "CREATED BOOKING".to_string(), format!("person=Member User, type=HIIT, datetime={}, location=Oak Hill Park, cost=1, credits_used=0", session_time))], read_logged(&pool).await);
    }

    #[sqlx::test]
    async fn cancel_booking_non_admin_trainer_of_session(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer_id = create_person(&pool, "Trainer User", "trainer@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "Member User", "member1@example.org", "member", 0).await;
        let session_time = Utc::now().fixed_offset().add(TimeDelta::days(1));
        let session_id = create_session(&pool, &session_time, trainer_id, "HIIT", "Oak Hill Park").await;
        create_booking(&pool, member_id, session_id, None).await;
        assert_eq!(1, count_bookings(&pool).await);

        let login = create_login(trainer_id, "trainer", "trainer");
        crate::bookings::delete_booking(State::from(&pool), State::from(&Config::load().unwrap()), State::from(&AppEnv::default()), login, member_id, session_id).await.unwrap();
        assert_eq!(0, count_bookings(&pool).await);

        assert_eq!(vec![("trainer@example.org".to_string(), "DELETED BOOKING".to_string(), format!("person=Member User, type=HIIT, datetime={}, location=Oak Hill Park, cost=1, credits_refunded=0", session_time))], read_logged(&pool).await);
    }

    #[sqlx::test]
    async fn cancel_booking_non_admin_trainer_not_of_session(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer1_id = create_person(&pool, "Test User", "trainer1@example.org", "member,trainer", 0).await;
        let trainer2_id = create_person(&pool, "Test User", "trainer2@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "Test User", "member1@example.org", "member", 0).await;
        let session_id = create_session(&pool, &Utc::now().fixed_offset().add(TimeDelta::days(1)), trainer1_id, "HIIT", "Oak Hill Park").await;
        create_booking(&pool, member_id, session_id, None).await;
        assert_eq!(1, count_bookings(&pool).await);

        let login = create_login(trainer2_id, "trainer2", "trainer");
        let result = crate::bookings::delete_booking(State::from(&pool), State::from(&Config::load().unwrap()), State::from(&AppEnv::default()), login, member_id, session_id).await;
        assert_eq!(Err(Custom(Status::Forbidden, "Not allowed to cancel bookings for other users.".to_string())), result);

        assert_eq!(0, read_logged(&pool).await.len());
    }

    #[sqlx::test]
    async fn book_session_non_admin_trainer_not_of_session(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer1_id = create_person(&pool, "Test User", "trainer1@example.org", "member,trainer", 0).await;
        let trainer2_id = create_person(&pool, "Test User", "trainer2@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "Test User", "member1@example.org", "member", 0).await;
        let session_id = create_session(&pool, &Utc::now().fixed_offset().add(TimeDelta::days(1)), trainer1_id, "HIIT", "Oak Hill Park").await;
        let booking = crate::bookings::SessionBooking {
            person_id: member_id,
            session_id,
            credits_used: None
        };

        // Create booking
        let login = create_login(trainer2_id, "trainer2", "trainer");
        let result = crate::bookings::create_booking(State::from(&pool), State::from(&Config::load().unwrap()), login, Json(booking)).await;
        assert_eq!(Err(Custom(Status::Forbidden, "Cannot create a booking for another user!".to_string())), result);

        assert_eq!(0, read_logged(&pool).await.len());
    }

    #[sqlx::test]
    async fn book_session_full_member(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer_id = create_person(&pool, "Trainer User", "trainer@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "Member User", "member@example.org", "member", 0).await;
        let session_time = Utc::now().fixed_offset().add(TimeDelta::days(1));
        let session_id = create_session(&pool, &session_time, trainer_id, "HIIT", "Oak Hill Park").await;
        let booking = crate::bookings::SessionBooking {
            person_id: member_id,
            session_id,
            credits_used: None
        };

        // Create booking
        let login = create_login(member_id, "member", "member");
        crate::bookings::create_booking(State::from(&pool), State::from(&Config::load().unwrap()), login, Json(booking)).await.unwrap();
        assert_eq!(1, count_bookings(&pool).await);

        assert_eq!(vec![("member@example.org".to_string(), "CREATED BOOKING".to_string(), format!("person=Member User, type=HIIT, datetime={}, location=Oak Hill Park, cost=1, credits_used=0", session_time))], read_logged(&pool).await);
    }

    #[sqlx::test]
    async fn book_session_non_member(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer_id = create_person(&pool, "Test User", "trainer@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "Test User", "member@example.org", "", 0).await;
        let session_id = create_session(&pool, &Utc::now().fixed_offset().add(TimeDelta::days(1)), trainer_id, "HIIT", "Oak Hill Park").await;
        let booking = crate::bookings::SessionBooking {
            person_id: member_id,
            session_id,
            credits_used: None
        };

        // Create booking
        let login = create_login(member_id, "test", "");
        let result = crate::bookings::create_booking(State::from(&pool), State::from(&Config::load().unwrap()), login, Json(booking)).await;
        assert!(result.is_err());
        assert_eq!(Custom(Status::Forbidden, "Missing or expired membership, and insufficient Pay As You Go credits.".to_string()), result.err().unwrap());

        // Postcondition: still zero bookings
        assert_eq!(0, count_bookings(&pool).await);

        assert_eq!(0, read_logged(&pool).await.len());
    }

    #[sqlx::test]
    async fn book_session_limited_member_existing_session_same_week(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer_id = create_person(&pool, "Trainer User", "trainer@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "Member User", "member@example.org", "limited-member", 0).await;
        let datetime = Utc::now().fixed_offset().add(TimeDelta::days(1));
        let session_id_1 = create_session(&pool, &datetime, trainer_id, "HIIT", "Oak Hill Park").await;
        let booking_1 = crate::bookings::SessionBooking {
            person_id: member_id,
            session_id: session_id_1,
            credits_used: None
        };
        let session_id_2 = create_session(&pool, &datetime, trainer_id, "On The Move", "Oak Hill Park").await;
        let booking_2 = crate::bookings::SessionBooking {
            person_id: member_id,
            session_id: session_id_2,
            credits_used: None
        };

        // Precondition: zero bookings
        assert_eq!(0, count_bookings(&pool).await);

        // Create booking 1
        let login = create_login(member_id, "member", "limited-member");
        crate::bookings::create_booking(State::from(&pool), State::from(&Config::load().unwrap()), login.clone(), Json(booking_1)).await.unwrap();

        // Postcondition 1: one booking
        assert_eq!(1, count_bookings(&pool).await);

        // Create booking 2: fails
        let result = crate::bookings::create_booking(State::from(&pool), State::from(&Config::load().unwrap()), login.clone(), Json(booking_2.clone())).await;
        assert!(result.is_err());
        assert_eq!(Custom(Status::Forbidden, "Cannot book session: member already has 1 booking(s) in this week.".to_string()), result.err().unwrap());

        // Postcondition 2: one booking
        assert_eq!(1, count_bookings(&pool).await);

        // Cancel booking 1
        crate::bookings::delete_booking(State::from(&pool), State::from(&Config::load().unwrap()), State::from(&AppEnv::default()), login.clone(), member_id, session_id_1).await.unwrap();

        // Postcondition 3: zero bookings
        assert_eq!(0, count_bookings(&pool).await);

        // Create booking 2: succeeds now
        crate::bookings::create_booking(State::from(&pool), State::from(&Config::load().unwrap()), login.clone(), Json(booking_2)).await.unwrap();

        // Postcondition 4: one booking
        assert_eq!(1, count_bookings(&pool).await);

        assert_eq!(vec![
            ("member@example.org".to_string(), "CREATED BOOKING".to_string(), format!("person=Member User, type=HIIT, datetime={}, location=Oak Hill Park, cost=1, credits_used=0", datetime)),
            ("member@example.org".to_string(), "DELETED BOOKING".to_string(), format!("person=Member User, type=HIIT, datetime={}, location=Oak Hill Park, cost=1, credits_refunded=0", datetime)),
            ("member@example.org".to_string(), "CREATED BOOKING".to_string(), format!("person=Member User, type=On The Move, datetime={}, location=Oak Hill Park, cost=1, credits_used=0", datetime))
        ], read_logged(&pool).await);
    }

    #[sqlx::test]
    async fn book_session_limited_member_existing_session_next_week(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer_id = create_person(&pool, "Trainer User", "trainer@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "Member User", "member@example.org", "limited-member", 0).await;
        let tomorrow = Utc::now().fixed_offset().add(TimeDelta::days(1));
        let next_week = tomorrow.add(TimeDelta::weeks(1));
        let session_id_1 = create_session(&pool, &tomorrow, trainer_id, "HIIT", "Oak Hill Park").await;
        let booking_1 = crate::bookings::SessionBooking {
            person_id: member_id,
            session_id: session_id_1,
            credits_used: None
        };
        let session_id_2 = create_session(&pool, &next_week, trainer_id, "On The Move", "Oak Hill Park").await;
        let booking_2 = crate::bookings::SessionBooking {
            person_id: member_id,
            session_id: session_id_2,
            credits_used: None
        };

        // Precondition: zero bookings
        assert_eq!(0, count_bookings(&pool).await);

        // Create booking 1
        let login = create_login(member_id, "member", "limited-member");
        crate::bookings::create_booking(State::from(&pool), State::from(&Config::load().unwrap()), login.clone(), Json(booking_1)).await.unwrap();

        // Postcondition 1: one booking
        assert_eq!(1, count_bookings(&pool).await);

        // Create booking 2: succeeds because it's next week
        crate::bookings::create_booking(State::from(&pool), State::from(&Config::load().unwrap()), login, Json(booking_2.clone())).await.unwrap();

        // Postcondition 2: two bookings
        assert_eq!(2, count_bookings(&pool).await);

        assert_eq!(vec![
            ("member@example.org".to_string(), "CREATED BOOKING".to_string(), format!("person=Member User, type=HIIT, datetime={}, location=Oak Hill Park, cost=1, credits_used=0", tomorrow)),
            ("member@example.org".to_string(), "CREATED BOOKING".to_string(), format!("person=Member User, type=On The Move, datetime={}, location=Oak Hill Park, cost=1, credits_used=0", next_week))
        ], read_logged(&pool).await);
    }

    #[sqlx::test]
    async fn book_session_non_member_using_credit_not_opted_in(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer_id = create_person(&pool, "Test User", "trainer@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "Test User", "member@example.org", "", 5).await;
        let session_id = create_session(&pool, &Utc::now().fixed_offset().add(TimeDelta::days(1)), trainer_id, "HIIT", "Oak Hill Park").await;
        let booking = crate::bookings::SessionBooking {
            person_id: member_id,
            session_id,
            credits_used: None
        };

        // Precondition: zero bookings
        assert_eq!(0, count_bookings(&pool).await);

        // Create booking
        let login = create_login(member_id, "nonmember", "");
        let result = crate::bookings::create_booking(State::from(&pool), State::from(&Config::load().unwrap()), login, Json(booking)).await;
        assert!(result.is_err());
        assert_eq!(Custom(Status::PaymentRequired, "Opt in to use credits for booking.".to_string()), result.err().unwrap());

        // Postcondition: still zero bookings
        assert_eq!(0, count_bookings(&pool).await);
        assert_eq!(0, read_logged(&pool).await.len());
    }

    #[sqlx::test]
    async fn book_session_non_member_using_credit_opted_in(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer_id = create_person(&pool, "Trainer User", "trainer@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "PAYG User", "member@example.org", "", 5).await;
        let session_datetime = Utc::now().fixed_offset().add(TimeDelta::days(1));
        let session_id = create_session(&pool, &session_datetime, trainer_id, "HIIT", "Oak Hill Park").await;
        let booking = crate::bookings::SessionBooking {
            person_id: member_id,
            session_id,
            credits_used: Some(1)
        };

        // Precondition: zero bookings
        assert_eq!(0, count_bookings(&pool).await);

        // Create booking
        let login = create_login(member_id, "PAYG", "");
        crate::bookings::create_booking(State::from(&pool), State::from(&Config::load().unwrap()), login.clone(), Json(booking)).await.expect("booking should be created");

        // Check that the booking has the used credits
        let created_booking: crate::bookings::SessionBooking = query_as("SELECT person_id, session_id, credits_used FROM booking WHERE person_id = $1 AND session_id = $2")
            .bind(member_id)
            .bind(session_id)
            .fetch_one(&pool)
            .await.unwrap();
        assert_eq!(Some(1), created_booking.credits_used);
        let bookings_list = crate::bookings::list_bookings(State::from(&pool), login.clone(), None, Some(member_id), None, None).await.unwrap();
        assert_eq!(1, bookings_list.len());
        assert_eq!(1, bookings_list.get(0).unwrap().credits_used);

        // Check that the user has been debited one credit
        let member_record = UserLoginRecord::load_by_id(&pool, member_id)
            .await.unwrap().unwrap();
        assert_eq!(4, member_record.credits);

        // Cancel booking
        crate::bookings::delete_booking(State::from(&pool), State::from(&Config::load().unwrap()), State::from(&AppEnv::default()), login.clone(), member_id, session_id).await.unwrap();
        // Postcondition: zero bookings
        assert_eq!(0, count_bookings(&pool).await);

        // Check that the user's credit has been restored
        let member_record = UserLoginRecord::load_by_id(&pool, member_id)
            .await.unwrap().unwrap();
        assert_eq!(5, member_record.credits);

        assert_eq!(vec![
            ("PAYG@example.org".to_string(), "CREATED BOOKING".to_string(), format!("person=PAYG User, type=HIIT, datetime={}, location=Oak Hill Park, cost=1, credits_used=1", session_datetime)),
            ("PAYG@example.org".to_string(), "DELETED BOOKING".to_string(), format!("person=PAYG User, type=HIIT, datetime={}, location=Oak Hill Park, cost=1, credits_refunded=1", session_datetime))
        ], read_logged(&pool).await);
    }

    #[sqlx::test]
    async fn book_session_non_member_using_credit_max_bookings_reached(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer_id = create_person(&pool, "Trainer User", "trainer@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "PAYG User", "member@example.org", "", 5).await;
        let session_id = create_session_max_bookings(&pool, &Utc::now().fixed_offset().add(TimeDelta::days(1)), trainer_id, "HIIT", "Oak Hill Park", Some(0)).await;
        let booking = crate::bookings::SessionBooking {
            person_id: member_id,
            session_id,
            credits_used: Some(1)
        };

        // Precondition: zero bookings
        assert_eq!(0, count_bookings(&pool).await);

        // Create booking: fail due to max bookings reached
        let login = create_login(member_id, "PAYG User", "");
        let booking_result = crate::bookings::create_booking(State::from(&pool), State::from(&Config::load().unwrap()), login, Json(booking)).await.err().unwrap();
        assert_eq!(Custom(Status::Conflict, "Session has reached it maximum number of bookings: 0.".to_string()), booking_result);

        // Still zero bookings
        assert_eq!(0, count_bookings(&pool).await);

        // Check that the user has NOT been debited any credits
        let member_record = UserLoginRecord::load_by_id(&pool, member_id)
            .await.unwrap().unwrap();
        assert_eq!(5, member_record.credits);

        assert_eq!(0, read_logged(&pool).await.len());
    }

    #[sqlx::test]
    async fn list_bookings_admin(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let now = DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z").unwrap();
        set_timestamp_datetime(&now);

        let admin_id = create_person(&pool, "Test User", "admin@example.org", "member,trainer", 0).await;
        let trainer_id = create_person(&pool, "Test User", "trainer@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "Test User", "member@example.org", "member", 0).await;
        let session_id = create_session(&pool, &Utc::now().fixed_offset().add(TimeDelta::days(1)), trainer_id, "HIIT", "Oak Hill Park").await;
        create_booking(&pool, member_id, session_id, None).await;

        let login = create_login(admin_id, "admin", "admin");
        let bookings = crate::bookings::list_bookings(State::from(&pool), login, Some(session_id), None, None, None).await.unwrap();

        assert_eq!(1,  bookings.len());
        assert_eq!(Some(now), bookings.get(0).unwrap().booked_timestamp);
    }

    #[sqlx::test]
    async fn list_bookings_non_admin(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer_id = create_person(&pool, "Test User", "trainer@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "Test User", "member@example.org", "member", 0).await;
        let session_id = create_session(&pool, &Utc::now().fixed_offset().add(TimeDelta::days(1)), trainer_id, "HIIT", "Oak Hill Park").await;
        create_booking(&pool, member_id, session_id, None).await;

        let login = create_login(member_id, "Test User", "");
        let result = crate::bookings::list_bookings(State::from(&pool), login, Some(session_id), None, None, None).await;

        assert_eq!(Err(Custom(Status::Forbidden, "user is not allowed to view bookings for selected person and/or session".to_string())), result);
    }

    #[sqlx::test]
    async fn list_bookings_non_admin_trainer_of_session(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer_id = create_person(&pool, "Test User", "trainer@example.org", "member,trainer", 0).await;
        let member_id = create_person(&pool, "Test User", "member@example.org", "member", 0).await;
        let session_id = create_session(&pool, &Utc::now().fixed_offset().add(TimeDelta::days(1)), trainer_id, "HIIT", "Oak Hill Park").await;
        create_booking(&pool, member_id, session_id, None).await;

        let login = create_login(trainer_id, "trainer", "trainer");
        let bookings = crate::bookings::list_bookings(State::from(&pool), login, Some(session_id), None, None, None).await.unwrap();

        assert_eq!(1, bookings.len());
    }

    #[sqlx::test]
    async fn list_bookings_non_admin_trainer_not_of_session(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer1_id = create_person(&pool, "Test User", "trainer1@example.org", "trainer", 0).await;
        let trainer2_id = create_person(&pool, "Test User", "trainer2@example.org", "trainer", 0).await;
        let member_id = create_person(&pool, "Test User", "member@example.org", "member", 0).await;
        let session_id = create_session(&pool, &Utc::now().fixed_offset().add(TimeDelta::days(1)), trainer1_id, "HIIT", "Oak Hill Park").await;
        create_booking(&pool, member_id, session_id, None).await;

        let login = create_login(trainer2_id, "trainer", "trainer");
        let result = crate::bookings::list_bookings(State::from(&pool), login, Some(session_id), None, None, None).await;

        assert_eq!(Err(Custom(Status::Forbidden, "user is not allowed to view bookings for selected person and/or session".to_string())), result);
    }

    #[sqlx::test]
    async fn mark_attendance_admin(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let admin_id = create_person(&pool, "Test User", "admin@example.org", "admin", 0).await;
        let trainer_id = create_person(&pool, "Test User", "trainer@example.org", "trainer", 0).await;
        let member_id = create_person(&pool, "Test User", "member@example.org", "member", 0).await;
        let session_id = create_session(&pool, &Utc::now().fixed_offset().add(TimeDelta::days(1)), trainer_id, "HIIT", "Oak Hill Park").await;
        create_booking(&pool, member_id, session_id, None).await;
        assert_eq!(0, count_bookings_attended(&pool, true).await);

        let login = create_login(admin_id,"admin", "admin");
        crate::bookings::update_booking(State::from(&pool), login, member_id, session_id, Json(BookingUpdate{attended: Some(true), feedback: None})).await.expect("booking update should succeed");
        assert_eq!(1, count_bookings_attended(&pool, true).await);
    }

    #[sqlx::test]
    async fn mark_attendance_non_admin(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer_id = create_person(&pool, "Test User", "trainer@example.org", "trainer", 0).await;
        let member_id = create_person(&pool, "Test User", "member@example.org", "member", 0).await;
        let session_id = create_session(&pool, &Utc::now().fixed_offset().add(TimeDelta::days(1)), trainer_id, "HIIT", "Oak Hill Park").await;
        create_booking(&pool, member_id, session_id, None).await;
        assert_eq!(0, count_bookings_attended(&pool, true).await);

        let login = create_login(member_id, "member", "member");
        let result = crate::bookings::update_booking(State::from(&pool), login, member_id, session_id, Json(BookingUpdate{attended: Some(true), feedback: None})).await;
        assert_eq!(Err(Custom(Status::Forbidden, "cannot update booking: must be the session trainer or an admin".to_string())), result);
        assert_eq!(0, count_bookings_attended(&pool, true).await);
    }

    #[sqlx::test]
    async fn mark_attendance_non_admin_trainer_of_session(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer_id = create_person(&pool, "Test User", "trainer@example.org", "trainer", 0).await;
        let member_id = create_person(&pool, "Test User", "member@example.org", "member", 0).await;
        let session_id = create_session(&pool, &Utc::now().fixed_offset().add(TimeDelta::days(1)), trainer_id, "HIIT", "Oak Hill Park").await;
        create_booking(&pool, member_id, session_id, None).await;
        assert_eq!(0, count_bookings_attended(&pool, true).await);

        let login = create_login(trainer_id, "trainer", "trainer");
        crate::bookings::update_booking(State::from(&pool), login, member_id, session_id, Json(BookingUpdate{attended: Some(true), feedback: None})).await.expect("booking update should succeed");
        assert_eq!(1, count_bookings_attended(&pool, true).await);
    }

    #[sqlx::test]
    async fn mark_attendance_non_admin_trainer_not_of_session(pool: PgPool) {
        pool.execute(include_str!("../schema.sql")).await.unwrap();

        let trainer1_id = create_person(&pool, "Test User", "trainer1@example.org", "trainer", 0).await;
        let trainer2_id = create_person(&pool, "Test User", "trainer2@example.org", "trainer", 0).await;
        let member_id = create_person(&pool, "Test User", "member@example.org", "member", 0).await;
        let session_id = create_session(&pool, &Utc::now().fixed_offset().add(TimeDelta::days(1)), trainer1_id, "HIIT", "Oak Hill Park").await;
        create_booking(&pool, member_id, session_id, None).await;
        assert_eq!(0, count_bookings_attended(&pool, true).await);

        let login = create_login(trainer2_id, "trainer2", "trainer");
        let result = crate::bookings::update_booking(State::from(&pool), login, member_id, session_id, Json(BookingUpdate{attended: Some(true), feedback: None})).await;
        assert_eq!(Err(Custom(Status::Forbidden, "cannot update booking: must be the session trainer or an admin".to_string())), result);
        assert_eq!(0, count_bookings_attended(&pool, true).await);
    }

    #[sqlx::test(fixtures("../schema.sql"))]
    async fn mark_rating_as_attendee_after_attended(pool: PgPool) {
        let admin_id = create_person(&pool, "Test User", "admin@example.org", "admin", 0).await;
        let trainer_id = create_person(&pool, "Test User", "trainer@example.org", "trainer", 0).await;
        let member_id = create_person(&pool, "Test User", "member@example.org", "member", 0).await;
        let session_id = create_session(&pool, &Utc::now().fixed_offset().add(TimeDelta::days(1)), trainer_id, "HIIT", "Oak Hill Park").await;
        create_booking(&pool, member_id, session_id, None).await;
        assert_eq!(0, count_bookings_attended(&pool, true).await);

        // Mark attended...
        crate::bookings::update_booking(
            State::from(&pool),
            create_login(admin_id, "admin", "admin"),
            member_id, session_id,
            Json(BookingUpdate{attended: Some(true), feedback: None})
        ).await.expect("booking update should succeed");
        assert_eq!(1, count_bookings_attended(&pool, true).await);

        // Rate session as booking attendee
        crate::bookings::update_booking(
            State::from(&pool),
            create_login(member_id, "member", "member"),
            member_id, session_id,
            Json(BookingUpdate{attended: None, feedback: Some(Feedback{ rating: 3, comment: Some("fun session!".to_string()) })})
        ).await.expect("booking update should succeed");

        let row = query("SELECT rating, comment FROM booking WHERE session_id = $1 AND person_id = $2")
            .bind(session_id)
            .bind(member_id)
            .fetch_one(&pool)
            .await.unwrap();
        assert_eq!(3, row.get::<i16, usize>(0));
        assert_eq!("fun session!", row.get::<&str, usize>(1));
    }

    #[sqlx::test(fixtures("../schema.sql"))]
    async fn delete_booking_waitlist_promoted(pool: PgPool) {
        // Setup users
        let admin_id = create_person(&pool, "Admin", "admin@example.com", "admin", 0).await;
        let admin_login = create_login(admin_id, "Admin", "admin");
        let trainer1_id = create_person(&pool, "Trainer1", "trainer1@example.org", "trainer", 0).await;
        let user1_id = create_person(&pool, "User1", "user1@example.com", "member", 0).await;
        let user1_login = create_login(user1_id, "User1", "member");
        let user2_id = create_person(&pool, "User2", "user2@example.com", "member", 0).await;
        let user2_login = create_login(user2_id, "User2", "member");

        // Create session with max bookings = 1 and 1 booked user
        let session_id = create_session_max_bookings(&pool, &Utc::now().fixed_offset().add(TimeDelta::days(1)), trainer1_id, "HIIT", "Oak Hill Park", Some(1)).await;
        create_booking(&pool, user1_id, session_id, None).await;
        
        // User2 tries to book => fails
        let config = Config::load().unwrap();
        let err = crate::bookings::create_booking(State::from(&pool), State::from(&config), user2_login.clone(), Json(SessionBooking { person_id: user2_id, session_id, credits_used: None })).await.unwrap_err();
        assert_eq!(Custom(Status::Conflict, "Session has reached it maximum number of bookings: 1.".to_string()), err);
        
        // User 2 joins waitlist
        add_waitlist(State::from(&pool), user2_login.clone(), Json(WaitlistEntry { person_id: user2_id, session_id })).await.unwrap();
        
        // User1 cancels his booking
        delete_booking(State::from(&pool), State::from(&Config::load().unwrap()), State::from(&AppEnv::default()), user1_login.clone(), user1_id, session_id).await.unwrap();

        // User2 is now booked
        let bookings = list_bookings(State::from(&pool), admin_login.clone(), Some(session_id), None, None, None).await.unwrap().0;
        assert_eq!(1, bookings.len(), "booking count for session should be 1 as waitlisted user was automatically promoted");
        assert_eq!(user2_id, bookings.get(0).unwrap().person_id);
        assert_eq!(0, list_waitlist(State::from(&pool), admin_login.clone(), session_id).await.unwrap().len(), "waitlist should be empty as user2 got a booking");
        let notification_emails = notifications::get_sent_messages();
        assert_eq!(1, notification_emails.len());
        assert_eq!("user2@example.com".to_string(), notification_emails.get(0).unwrap().0);
        assert_eq!("The wait is over, your session booking has been confirmed!".to_string(), notification_emails.get(0).unwrap().1);
    }

    #[sqlx::test(fixtures("../schema.sql"))]
    async fn delete_booking_waitlist_promotion_failed_no_credits(pool: PgPool) {
        // Setup users
        let admin_id = create_person(&pool, "Admin", "admin@example.com", "admin", 0).await;
        let admin_login = create_login(admin_id, "Admin", "admin");
        let trainer1_id = create_person(&pool, "Trainer1", "trainer1@example.org", "trainer", 0).await;
        let user1_id = create_person(&pool, "User1", "user1@example.com", "member", 0).await;
        let user1_login = create_login(user1_id, "User1", "member");
        let user2_id = create_person(&pool, "User2", "user2@example.com", "", 0).await;
        let user2_login = create_login(user2_id, "User2", "");

        // Create session with max bookings = 1 and 1 booked user
        let session_id = create_session_max_bookings(&pool, &Utc::now().fixed_offset().add(TimeDelta::days(1)), trainer1_id, "HIIT", "Oak Hill Park", Some(1)).await;
        create_booking(&pool, user1_id, session_id, None).await;
        
        // User 2 joins waitlist
        add_waitlist(State::from(&pool), user2_login.clone(), Json(WaitlistEntry { person_id: user2_id, session_id })).await.unwrap();
        
        // User1 cancels his booking
        delete_booking(State::from(&pool), State::from(&Config::load().unwrap()), State::from(&AppEnv::default()), user1_login.clone(), user1_id, session_id).await.unwrap();

        // User2 is NOT now booked because he doesn't have enough credits
        let bookings = list_bookings(State::from(&pool), admin_login.clone(), Some(session_id), None, None, None).await.unwrap().0;
        assert_eq!(0, bookings.len());
        assert_eq!(0, list_waitlist(State::from(&pool), admin_login.clone(), session_id).await.unwrap().len(), "waitlist should be empty");

        let notification_emails = notifications::get_sent_messages();
        assert_eq!(1, notification_emails.len());
        assert_eq!("user2@example.com".to_string(), notification_emails.get(0).unwrap().0);
        assert_eq!("We were unable to confirm your booking.".to_string(), notification_emails.get(0).unwrap().1);
    }

    #[sqlx::test(fixtures("../schema.sql"))]
    async fn delete_booking_waitlist_promotion_failed_no_credits_next_on_waitlist_books(pool: PgPool) {
        // Setup users
        let admin_id = create_person(&pool, "Admin", "admin@example.com", "admin", 0).await;
        let admin_login = create_login(admin_id, "Admin", "admin");
        let trainer1_id = create_person(&pool, "Trainer1", "trainer1@example.org", "trainer", 0).await;
        let user1_id = create_person(&pool, "User1", "user1@example.com", "member", 0).await;
        let user1_login = create_login(user1_id, "User1", "member");
        let user2_id = create_person(&pool, "User2", "user2@example.com", "", 0).await; // No membership, no PAYG credits
        let user2_login = create_login(user2_id, "User2", "");
        let user3_id = create_person(&pool, "User3", "user3@example.com", "", 1).await; // No membership but sufficient PAYG credits
        let user3_login = create_login(user3_id, "User3", "member");

        // Create session with max bookings = 1 and 1 booked user
        let session_id = create_session_max_bookings(&pool, &Utc::now().fixed_offset().add(TimeDelta::days(1)), trainer1_id, "HIIT", "Oak Hill Park", Some(1)).await;
        create_booking(&pool, user1_id, session_id, None).await;
        
        // Users 2 & 3 joins waitlist
        add_waitlist(State::from(&pool), user2_login.clone(), Json(WaitlistEntry { person_id: user2_id, session_id })).await.unwrap();
        add_waitlist(State::from(&pool), user3_login.clone(), Json(WaitlistEntry { person_id: user3_id, session_id })).await.unwrap();
        
        // User1 cancels his booking
        delete_booking(State::from(&pool), State::from(&Config::load().unwrap()), State::from(&AppEnv::default()), user1_login.clone(), user1_id, session_id).await.unwrap();

        // User 2 is NOT now booked because he doesn't have enough credits. User 3 is booked and the waitlist is empty.
        let bookings = list_bookings(State::from(&pool), admin_login.clone(), Some(session_id), None, None, None).await.unwrap().0;
        assert_eq!(1, bookings.len(), "should be 1 booking for session");
        assert_eq!(user3_id, bookings.get(0).unwrap().person_id);
        assert_eq!(0, list_waitlist(State::from(&pool), admin_login.clone(), session_id).await.unwrap().len(), "waitlist should be empty");
        assert_eq!(Some::<i16>(0), query("SELECT credits FROM person WHERE id = $1").bind(user3_id).fetch_one(&pool).await.and_then(|r| r.try_get("credits")).ok(), "user3 credit count should have been reduced to 0");

        // Verify that user2 received "failed to book" email and user3 received "booked" email
        let notification_emails = notifications::get_sent_messages();
        assert_eq!(2, notification_emails.len());
        assert_eq!("user2@example.com".to_string(), notification_emails.get(0).unwrap().0);
        assert_eq!("We were unable to confirm your booking.".to_string(), notification_emails.get(0).unwrap().1);
        assert_eq!("user3@example.com".to_string(), notification_emails.get(1).unwrap().0);
        assert_eq!("The wait is over, your session booking has been confirmed!".to_string(), notification_emails.get(1).unwrap().1);
    }

    fn create_login(uid: i64, name: &str, role: &str) -> LoginSession {
        LoginSession {
            sessionid: "xxx".to_string(),
            uid,
            name: name.to_string(),
            email: format!("{}@example.org", name),
            roles: Roles::parse(role),
            loggedin: None, loggedin_from: None,
            expiry: Utc::now().checked_add_days(Days::new(1)).unwrap().fixed_offset()
        }
    }

}

