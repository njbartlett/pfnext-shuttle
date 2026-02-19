use chrono::{NaiveDate, TimeZone};
use chrono_tz::Tz;
use futures::future::try_join_all;
use rocket::http::Status;
use rocket::response::status::{Created, Custom, NoContent};
use rocket::serde::json::Json;
use rocket::{Route, State};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{query_as, query_scalar, Error, FromRow, PgPool, Postgres, QueryBuilder, Row};

use crate::common::parse_opt_naive_date;
use crate::config::Config;
use crate::loginsession::LoginSession;
use crate::whereclause::Operator::{Equal, GreaterThan, GreaterThanOrEqual, LessThanOrEqual};
use crate::whereclause::WhereClause;

#[cfg(test)]
use crate::mock_chrono::Utc;

#[cfg(not(test))]
use chrono::Utc;

const DATE_FORMAT: &str = "%Y-%m-%d";

pub fn routes() -> Vec<Route> {
    routes![
        list_activity_types,
        list_challenges,
        get_challenge,
        get_activity,
        list_activities,
        create_activity,
        delete_activity,
    ]
}

#[derive(FromRow, Serialize, Deserialize, Clone, Debug, PartialEq)]
struct ActivityType {
    id: i32,
    name: String,
    units: String,
    step_size: f32
}

impl ActivityType {
    async fn query(pool: &PgPool) -> Result<Vec<ActivityType>, Error> {
        query_as("SELECT id, name, units, step_size FROM activity_type ORDER BY id ASC")
            .fetch_all(pool)
            .await
    }
}

#[derive(Clone, Debug, Deserialize, FromRow, PartialEq, Serialize)]
struct ChallengeRecord {
    id: i64,
    name: String,
    description: Option<String>,
    start: NaiveDate,
    finish: NaiveDate,
    activity_type: ActivityType,
    goal: f32,
    individual_goal: Option<f32>,
    daily_goal: Option<f32>,
    total_all: f32,
    total_for_person: f32
}

impl ChallengeRecord {
    fn _create_query<'a>(
        challenge_id: Option<i64>,
        person_id: Option<i64>,
    ) -> QueryBuilder<'a, Postgres> {
        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new("SELECT \
                c.id, c.name, c.description, c.start, c.finish, c.goal, c.individual_goal, c.daily_goal, \
                a.id AS activity_type_id, a.name AS activity_type_name, a.units AS activity_type_units, a.step_size AS activity_type_step_size, \
                (SELECT COALESCE(SUM(amount), 0) FROM activity WHERE challenge_id = c.id) AS activity_total_all"
        );
        if let Some(person_id) = person_id {
            qb.push(", (SELECT COALESCE(SUM(amount), 0) FROM activity WHERE challenge_id = c.id AND person_id = ");
            qb.push_bind(person_id);
            qb.push(") AS activity_total_for_person");
        }
        qb.push(" FROM challenge AS c JOIN activity_type AS a ON c.activity_type = a.id");
        
        WhereClause::init().opt_append_to(&mut qb, "c.id", Equal, challenge_id);    
        qb
    }
    async fn list(
        pool: &PgPool,
        person_id: Option<i64>
    ) -> Result<Vec<Self>, Error> {
        Self::_create_query(None, person_id)
            .build_query_as()
            .fetch_all(pool)
            .await
    }
    async fn query_by_id(
        pool: &PgPool,
        challenge_id: i64,
        person_id: Option<i64>
    ) -> Result<Self, Error> {
        let mut query = Self::_create_query(Some(challenge_id), person_id);
        query.build_query_as()
            .fetch_one(pool)
            .await
    }
}

impl FromRow<'_, PgRow> for ChallengeRecord {
    fn from_row(row: &'_ PgRow) -> Result<Self, Error> {
        Ok(ChallengeRecord {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            description: row.try_get("description")?,
            start: row.try_get("start")?,
            finish: row.try_get("finish")?,
            goal: row.try_get("goal")?,
            individual_goal: row.try_get("individual_goal")?,
            daily_goal: row.try_get("daily_goal")?,
            total_all: row.try_get("activity_total_all")?,
            total_for_person: row.try_get("activity_total_for_person").unwrap_or(0.0),
            activity_type: ActivityType {
                id: row.try_get("activity_type_id")?,
                name: row.try_get("activity_type_name")?,
                units: row.try_get("activity_type_units")?,
                step_size: row.try_get("activity_type_step_size")?,
            },
        })
    }
}


#[derive(Serialize, Clone, FromRow, Debug)]
struct MemberActivitySummary {
    id: Option<i64>,
    name: Option<String>,
    total_amount: f32
}

impl MemberActivitySummary {
    async fn query(pool: &PgPool, challenge_id: i64, limit: &Option<i32>) -> Result<Vec<MemberActivitySummary>, Error> {
        let mut qb = QueryBuilder::new("SELECT p.id, p.name, SUM(a.amount) AS total_amount \
            FROM activity AS a \
            JOIN person AS p ON a.person_id = p.id");
        qb.push(" WHERE a.challenge_id = ");
        qb.push_bind(challenge_id);
        qb.push(" GROUP BY p.id, p.name ORDER BY total_amount DESC");
        if let Some(limit) = limit {
            qb.push(" LIMIT ");
            qb.push_bind(limit);
        }
        
        let query = qb.build_query_as();
            query.fetch_all(pool)
            .await
    }
}

#[derive(Serialize, Debug)]
struct ChallengeFull {
    id: i64,
    name: String,
    description: Option<String>,
    start: NaiveDate,
    finish: NaiveDate,
    activity_type: ActivityType,
    goal: f32,
    individual_goal: Option<f32>,
    total_all: f32,
    total_for_person: f32,
    member_summaries: Vec<MemberActivitySummary>
}

const TOLERANCE: f32 = 0.00001;

impl ChallengeFull {
    fn copy_record(record: &ChallengeRecord) -> Self {
        Self {
            id: record.id,
            name: record.name.clone(),
            description: record.description.clone(),
            start: record.start.clone(),
            finish: record.finish.clone(),
            activity_type: record.activity_type.clone(),
            goal: record.goal,
            individual_goal: record.individual_goal,
            total_all: record.total_all,
            total_for_person: record.total_for_person,
            member_summaries: vec![]
        }
    }
    async fn expand_leaderboard(mut self, pool: &PgPool, limit: &Option<i32>) -> Result<Self, Error> {
        let mut leaderboard = MemberActivitySummary::query(pool, self.id, limit).await?;
        if limit.is_some() {
            let leaderboard_sum: f32 = leaderboard.iter()
                .map(|s| s.total_amount)
                .sum();
            let remainder = self.total_all - leaderboard_sum;
            if remainder >= TOLERANCE {
                leaderboard.push(MemberActivitySummary {
                    id: None, name: None, total_amount: remainder
                });
            }
        }
        self.member_summaries = leaderboard;
        Ok(self)
    }
}

#[get("/activity_types")]
async fn list_activity_types(pool: &State<PgPool>) -> Result<Json<Vec<ActivityType>>, Custom<String>> {
    ActivityType::query(pool)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
        .map(Json::from)
}

#[derive(Serialize, Deserialize, Debug)]
struct NewActivity {
    person_id: i64,
    challenge_id: Option<i64>,
    activity_type: Option<i32>,
    date: NaiveDate,
    amount: f32
}

impl NewActivity {
    async fn save(&self, pool: &PgPool) -> Result<i64, Error> {
        if let Some(activity_type) = self.activity_type {

            // Check that the challenge ID activity matches
            if let Some(challenge_id) = self.challenge_id {
                let challenge_activity_type: i32 = query_scalar("SELECT activity_type FROM challenge WHERE id = $1")
                    .bind(challenge_id)
                    .fetch_one(pool)
                    .await?;
                if activity_type != challenge_activity_type {
                    return Err(Error::InvalidArgument("activity_type clashes with challenge activity type".to_string()))
                }
            }

            let sql = "INSERT INTO activity (person_id, challenge_id, activity_type, date, amount) VALUES ($1, $2, $3, $4, $5) RETURNING id";
            query_scalar(sql)
                .bind(self.person_id)
                .bind(self.challenge_id)
                .bind(activity_type)
                .bind(self.date)
                .bind(self.amount)
                .fetch_one(pool)
                .await
        } else if let Some(challenge_id) = self.challenge_id {
            let sql = "INSERT INTO activity (person_id, challenge_id, activity_type, date, amount) 
                    SELECT  $1, $2, activity_type, $3, $4
                    FROM challenge WHERE challenge.id = $2
                    RETURNING id";
            query_scalar(sql)
                .bind(self.person_id)
                .bind(challenge_id)
                .bind(self.date)
                .bind(self.amount)
                .fetch_one(pool)
                .await
        } else {
            Err(Error::InvalidArgument("either challenge_id or activity_type required".to_string()))
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct Activity {
    id: i64,
    person_id: i64,
    person_name: String,
    person_email: String,
    challenge: Option<ChallengeRecord>,
    activity_type: ActivityType,
    date: NaiveDate,
    amount: f32
}

impl FromRow<'_, PgRow> for Activity {
    fn from_row(r: &'_ PgRow) -> Result<Self, Error> {
        let challenge = if let Some(challenge_id) = r.try_get("challenge_id")? {
            Some(ChallengeRecord {
                id: challenge_id,
                name: r.try_get("challenge_name")?,
                description: r.try_get("challenge_description")?,
                start: r.try_get("challenge_start")?,
                finish: r.try_get("challenge_finish")?,
                activity_type: ActivityType {
                    id: r.try_get("challenge_activity_type_id")?,
                    name: r.try_get("challenge_activity_type_name")?,
                    units: r.try_get("challenge_activity_type_units")?,
                    step_size: r.try_get("challenge_activity_type_step_size")?
                },
                goal: r.try_get("challenge_goal")?,
                individual_goal: r.try_get("challenge_individual_goal")?,
                daily_goal: r.try_get("challenge_daily_goal")?,
                total_all: 0.0, total_for_person: 0.0
            })
        } else {
            None
        };
        Ok(Activity {
            id: r.try_get("id")?,
            person_id: r.try_get("person_id")?,
            person_name: r.try_get("person_name")?,
            person_email: r.try_get("person_email")?,
            challenge: challenge,
            activity_type: ActivityType {
                id: r.try_get("activity_type_id")?,
                name: r.try_get("activity_type_name")?,
                units: r.try_get("activity_type_units")?,
                step_size: r.try_get("activity_type_step_size")?
            },
            date: r.try_get("date")?,
            amount: r.try_get("amount")?
        })
    }
}

impl Activity {

    const ACTIVITY_QUERY_BASE: &str = "SELECT a.id, a.person_id, a.date, a.amount,
            p.name AS person_name, p.email AS person_email,
            c.id AS challenge_id, c.name AS challenge_name, c.description AS challenge_description, c.start AS challenge_start, c.finish AS challenge_finish, c.goal AS challenge_goal, c.individual_goal AS challenge_individual_goal, c.daily_goal AS challenge_daily_goal,
            ct.id AS challenge_activity_type_id, ct.name AS challenge_activity_type_name, ct.units AS challenge_activity_type_units, ct.step_size AS challenge_activity_type_step_size,
            t.id AS activity_type_id, t.name AS activity_type_name, t.units AS activity_type_units, t.step_size AS activity_type_step_size
        FROM activity AS a
        INNER JOIN person AS p ON a.person_id = p.id
        LEFT JOIN challenge AS c ON a.challenge_id = c.id
        LEFT JOIN activity_type AS ct ON c.activity_type = ct.id
        LEFT JOIN activity_type AS t ON a.activity_type = t.id";

    async fn query(
        pool: &PgPool,
        challenge_id: Option<i64>,
        person_id: Option<i64>,
        from: Option<NaiveDate>,
        to: Option<NaiveDate>,
        activity_type: Option<i32>
    ) -> Result<Vec<Activity>, Error> {
        let mut qb = QueryBuilder::new(Self::ACTIVITY_QUERY_BASE);
        WhereClause::init()
            .opt_append_to(&mut qb, "a.challenge_id", Equal, challenge_id)
            .opt_append_to(&mut qb, "a.person_id", Equal, person_id)
            .opt_append_to(&mut qb, "a.date", GreaterThanOrEqual, from)
            .opt_append_to(&mut qb, "a.date", LessThanOrEqual, to)
            .opt_append_to(&mut qb, "a.activity_type", Equal, activity_type);
        qb.push(" ORDER BY a.date DESC, a.id DESC");
        qb.build_query_as().fetch_all(pool).await
    }

    async fn query_by_id(pool: &PgPool, id: i64) -> Result<Option<Activity>, Error> {
        let mut qb = QueryBuilder::new(Self::ACTIVITY_QUERY_BASE);
        WhereClause::init().append_to(&mut qb, "a.id", Equal, id);
        qb.build_query_as()
            .fetch_optional(pool)
            .await
    }
}

#[get("/activities?<challenge_id>&<person_id>&<from>&<to>&<activity_type>")]
async fn list_activities(
    pool: &State<PgPool>,
    login: LoginSession,
    challenge_id: Option<i64>,
    person_id: Option<i64>,
    from: Option<String>,
    to: Option<String>,
    activity_type: Option<i32>
) -> Result<Json<Vec<Activity>>, Custom<String>> {
    if !login.is_admin() && Some(login.uid) != person_id {
        return Err(Custom(Status::Forbidden, "admin role required to view other user activities".to_string()));
    }
    Activity::query(pool, challenge_id, person_id, parse_opt_naive_date(from, DATE_FORMAT)?, parse_opt_naive_date(to, DATE_FORMAT)?, activity_type)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
        .map(Json::from)
}

fn check_challenge_permission(login: &LoginSession, person_id: &Option<i64>) -> Result<(), Custom<String>> {
    if let Some(person_id) = person_id {
        if !login.is_admin() && login.uid != person_id.clone() {
            return Err(Custom(Status::Forbidden, "admin role required to view other user challenge totals".to_string()));
        }
    }
    Ok(())
}

#[get("/challenges?<person_id>&<date_today>&<leaderboard_limit>")]
async fn list_challenges(
    pool: &State<PgPool>,
    config: &State<Config>,
    login: LoginSession,
    person_id: Option<i64>,
    date_today: Option<String>,
    leaderboard_limit: Option<i32>
) -> Result<Json<Vec<ChallengeFull>>, Custom<String>> {
    check_challenge_permission(&login, &person_id)?;
    
    // Set date_now to the current time clock if not specified as a parameter
    let today = if let Some(date_today) = date_today {
        NaiveDate::parse_from_str(&date_today, DATE_FORMAT).map_err(|e| Custom(Status::BadRequest, e.to_string()))?
    } else {
        let tz: Tz = config.get_timezone().map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
        tz.from_utc_datetime(&Utc::now().naive_utc()).date_naive()
    };

    let records = ChallengeRecord::list(pool, person_id)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;

    try_join_all(records.iter().map(|r| expand_record_if_active(&pool, r, &today, &leaderboard_limit)))
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
        .map(Json::from)
}

async fn expand_record_if_active(pool: &PgPool, r: &ChallengeRecord, today: &NaiveDate, leaderboard_limit: &Option<i32>) -> Result<ChallengeFull, Error> {
    let full = ChallengeFull::copy_record(r);
    if today >= &full.start {
        full.expand_leaderboard(pool, leaderboard_limit).await
    } else {
        Ok(full)
    }
}

#[get("/challenges/<id>?<person_id>&<leaderboard_limit>")]
async fn get_challenge(
    pool: &State<PgPool>,
    login: LoginSession,
    id: i64,
    person_id: Option<i64>,
    leaderboard_limit: Option<i32>
) -> Result<Json<ChallengeFull>, Custom<String>> {
    check_challenge_permission(&login, &person_id)?;
    let simple_record: ChallengeRecord = ChallengeRecord::query_by_id(pool, id, person_id)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;

    ChallengeFull::copy_record(&simple_record).expand_leaderboard(pool, &leaderboard_limit)
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
        .map(Json::from)
}

#[get("/activities/<activity_id>")]
async fn get_activity(
    pool: &State<PgPool>,
    login: LoginSession,
    activity_id: i64
) -> Result<Json<Activity>, Custom<String>> {
    let activity = Activity::query_by_id(pool, activity_id).await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
        .and_then(|o: Option<Activity>| o.ok_or(Custom(Status::NotFound, "activity not found".to_string())))?;
    if !login.is_admin() && login.uid != activity.person_id {
        return Err(Custom(Status::Forbidden, "admin role required to view other user activities".to_string()));
    }

    Ok(Json::from(activity))
}

#[post("/activities", data = "<activity>")]
async fn create_activity(
    pool: &State<PgPool>,
    login: LoginSession,
    activity: Json<NewActivity>
) -> Result<Created<&'static str>, Custom<String>> {
    if !login.is_admin() && activity.person_id != login.uid {
        return Err(Custom(Status::Forbidden, "admin role required to create activities for other users".to_owned()));
    }  
    let activity_id = activity.save(pool).await
        .map_err(|e| match e {
            sqlx::Error::InvalidArgument(msg) => Custom(Status::UnprocessableEntity, msg.to_string()),
            _ => Custom(Status::InternalServerError, e.to_string())
        })?;
    Ok(Created::new(format!("/activities/{}", activity_id)))
}

#[delete("/activities/<activity_id>")]
async fn delete_activity(
    pool: &State<PgPool>,
    login: LoginSession,
    activity_id: i64
) -> Result<NoContent, Custom<String>> {
    let mut qb = QueryBuilder::new("DELETE FROM activity");
    let mut wc = WhereClause::init();
    
    wc.append_to(&mut qb, "id", Equal, activity_id);
    if !login.is_admin() {
        wc.append_to(&mut qb, "person_id", Equal, login.uid);
    }
    qb.push(" RETURNING id");

    qb.build()
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
        .and_then(|r| r.ok_or(Custom(Status::NotFound, "activity not found or user not allowed to delete".to_string())))
        .map(|_| NoContent)
}


#[cfg(test)]
mod tests {
    use chrono::{Days, NaiveDate, Utc};
    use rocket::http::Status;
    use rocket::response::status::Custom;
    use rocket::serde::json::Json;
    use rocket::State;
    use sqlx::{query_scalar, PgPool, Row};

    use crate::activities::{get_activity, list_activities, Activity, ChallengeFull, NewActivity};
    use crate::config::Config;
    use crate::loginsession::{LoginSession, Roles};
    use crate::mock_chrono::set_timestamp_rfc3339;
    use crate::testcommon::{find_user_login_by_name, find_person_id_by_name};

    async fn find_challenge_by_name(pool: &PgPool, name: &str) -> i64 {
        query_scalar("SELECT id FROM challenge WHERE name = $1")
            .bind(name)
            .fetch_one(pool)
            .await
            .expect(&format!("failed to find challenge with name {}", name))
    }

    async fn find_activity_type(pool: &PgPool, name: &str) -> i32 {
        query_scalar("SELECT id FROM activity_type WHERE name = $1")
            .bind(name)
            .fetch_one(pool)
            .await
            .unwrap()
    }
    
    async fn count_activities(pool: &PgPool) -> i64 {
        query_scalar("SELECT COUNT(*) FROM activity")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    #[sqlx::test(fixtures("../schema.sql"))]
    async fn test_list_activity_types(pool: PgPool) {
        let list = crate::activities::list_activity_types(State::from(&pool)).await.unwrap();
        assert_eq!(6, list.len());
        assert_eq!("Hiking", list[0].name);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql", "fixtures/activities.sql"))]
    async fn test_list_challenges_anon_admin(pool: PgPool) {
        let challenges: Vec<ChallengeFull> = crate::activities::list_challenges(
            State::from(&pool),
            State::from(&Config::load().unwrap()),
            find_user_login_by_name(&pool, "admin", "admin").await,
            None,
            Some("2025-05-15".to_string()),
            None
        ).await.unwrap().0;
        assert_eq!(4, challenges.len());

        assert_eq!("April 2025 Hikes", challenges[0].name);
        assert_eq!(2, challenges[0].member_summaries.len());
        assert_eq!(700.0, challenges[0].total_all);
        assert_eq!(0.0, challenges[0].total_for_person);

        assert_eq!("May 2025 Cycling", challenges[1].name);
        assert_eq!(1, challenges[1].member_summaries.len());
        assert_eq!(100.0, challenges[1].total_all);
        assert_eq!(0.0, challenges[1].total_for_person);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql", "fixtures/activities.sql"))]
    async fn test_list_challenges_anon_nonadmin(pool: PgPool) {
        let challenges: Vec<ChallengeFull> = crate::activities::list_challenges(
            State::from(&pool),
            State::from(&Config::load().unwrap()),
            find_user_login_by_name(&pool, "user1", "member").await,
            None,
            Some("2025-04-15".to_string()),
            None
        ).await.unwrap().0;
        assert_eq!(4, challenges.len());

        assert_eq!("April 2025 Hikes", challenges[0].name);
        assert_eq!(2, challenges[0].member_summaries.len());
        assert_eq!(700.0, challenges[0].total_all);
        assert_eq!(0.0, challenges[0].total_for_person);

        assert_eq!("May 2025 Cycling", challenges[1].name);
        assert_eq!(0, challenges[1].member_summaries.len());
        assert_eq!(100.0, challenges[1].total_all);
        assert_eq!(0.0, challenges[1].total_for_person);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql", "fixtures/activities.sql"))]
    async fn test_list_challenges_for_user(pool: PgPool) {
        let challenges: Vec<ChallengeFull> = crate::activities::list_challenges(
            State::from(&pool),
            State::from(&Config::load().unwrap()),
            find_user_login_by_name(&pool, "user1", "member").await,
            Some(find_person_id_by_name(&pool, "user1").await),
            Some("2025-04-15".to_string()),
            None
        ).await.unwrap().0;
        assert_eq!(4, challenges.len());

        assert_eq!("April 2025 Hikes", challenges[0].name);
        assert_eq!(2, challenges[0].member_summaries.len());
        assert_eq!(700.0, challenges[0].total_all);
        assert_eq!(300.0, challenges[0].total_for_person);

        assert_eq!("May 2025 Cycling", challenges[1].name);
        assert_eq!(0, challenges[1].member_summaries.len());
        assert_eq!(100.0, challenges[1].total_all);
        assert_eq!(100.0, challenges[1].total_for_person);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql", "fixtures/activities.sql"))]
    async fn test_list_challenges_for_otheruser_admin(pool: PgPool) {
        let challenges: Vec<ChallengeFull> = crate::activities::list_challenges(
            State::from(&pool),
            State::from(&Config::load().unwrap()),
            find_user_login_by_name(&pool, "admin", "admin").await,
            Some(find_person_id_by_name(&pool, "user1").await),
            Some("2025-04-15".to_string()),
            None
        ).await.unwrap().0;
        assert_eq!(4, challenges.len());

        assert_eq!("April 2025 Hikes", challenges[0].name);
        assert_eq!(2, challenges[0].member_summaries.len());
        assert_eq!(700.0, challenges[0].total_all);
        assert_eq!(300.0, challenges[0].total_for_person);

        assert_eq!("May 2025 Cycling", challenges[1].name);
        assert_eq!(0, challenges[1].member_summaries.len());
        assert_eq!(100.0, challenges[1].total_all);
        assert_eq!(100.0, challenges[1].total_for_person);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql", "fixtures/activities.sql"))]
    async fn test_list_challenges_system_date_before(pool: PgPool) {
        set_timestamp_rfc3339("1970-01-01T00:00:00Z");

        let challenges: Vec<ChallengeFull> = crate::activities::list_challenges(
            State::from(&pool),
            State::from(&Config::load().unwrap()),
            find_user_login_by_name(&pool, "admin", "admin").await,
            Some(find_person_id_by_name(&pool, "user1").await),
            None,
            None
        ).await.unwrap().0;
        assert_eq!(4, challenges.len());

        for c in challenges {
            assert_eq!(0, c.member_summaries.len());
        }
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql", "fixtures/activities.sql"))]
    async fn test_list_challenges_system_date_after(pool: PgPool) {
        set_timestamp_rfc3339("2040-01-01T00:00:00Z");

        let challenges: Vec<ChallengeFull> = crate::activities::list_challenges(
            State::from(&pool),
            State::from(&Config::load().unwrap()),
            find_user_login_by_name(&pool, "admin", "admin").await,
            Some(find_person_id_by_name(&pool, "user1").await),
            None,
            None
        ).await.unwrap().0;
        assert_eq!(4, challenges.len());
        assert_eq!(2, challenges[0].member_summaries.len());
        assert_eq!(1, challenges[1].member_summaries.len());
        assert_eq!(0, challenges[2].member_summaries.len());
    }


    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql", "fixtures/activities.sql"))]
    async fn test_list_challenges_limit_leaderboard(pool: PgPool) {
        set_timestamp_rfc3339("2040-01-01T00:00:00Z");

        let challenges: Vec<ChallengeFull> = crate::activities::list_challenges(
            State::from(&pool),
            State::from(&Config::load().unwrap()),
            find_user_login_by_name(&pool, "admin", "admin").await,
            Some(find_person_id_by_name(&pool, "user1").await),
            None,
            Some(1)
        ).await.unwrap().0;
        assert_eq!(4, challenges.len());
        assert_eq!(2, challenges[0].member_summaries.len());

        assert_eq!(Some("user2".to_string()), challenges[0].member_summaries[0].name);
        assert_eq!(400.0, challenges[0].member_summaries[0].total_amount);

        assert_eq!(None, challenges[0].member_summaries[1].name);
        assert_eq!(300.0, challenges[0].member_summaries[1].total_amount);

        assert_eq!(1, challenges[1].member_summaries.len());
        assert_eq!(0, challenges[2].member_summaries.len());
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql", "fixtures/activities.sql"))]
    async fn test_list_challenges_system_date_using_configured_timezone1(pool: PgPool) {
        // Set timezone to New York
        let mut config = Config::load().unwrap();
        config.timezone_name = "America/New_York".to_string();

        // Set current time to 00:00 on 1 April 2025 in UTC, which is 20:00 on 31 March in New York
        set_timestamp_rfc3339("2025-04-01T00:00:00Z");

        let challenges: Vec<ChallengeFull> = crate::activities::list_challenges(
            State::from(&pool),
            State::from(&config),
            find_user_login_by_name(&pool, "admin", "admin").await,
            Some(find_person_id_by_name(&pool, "user1").await),
            None,
            None
        ).await.unwrap().0;
        assert_eq!(4, challenges.len());

        // No challenges expanded because in our configured timezone, none of them have started
        for c in challenges {
            assert_eq!(0, c.member_summaries.len());
        }
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql", "fixtures/activities.sql"))]
    async fn test_list_challenges_system_date_using_configured_timezone2(pool: PgPool) {
        // Set timezone to London
        let mut config = Config::load().unwrap();
        config.timezone_name = "Europe/London".to_string();

        // Set current time to 23:00 on 31 March 2025 in UTC, which is 00:00 on 1 April in London (BST)
        set_timestamp_rfc3339("2025-03-31T23:00:00Z");

        let challenges: Vec<ChallengeFull> = crate::activities::list_challenges(
            State::from(&pool),
            State::from(&config),
            find_user_login_by_name(&pool, "admin", "admin").await,
            Some(find_person_id_by_name(&pool, "user1").await),
            None,
            None
        ).await.unwrap().0;
        assert_eq!(4, challenges.len());
        // First challenge is expanded because it has started in our configured timezone
        assert_eq!(2, challenges[0].member_summaries.len());
        assert_eq!(0, challenges[1].member_summaries.len());
        assert_eq!(0, challenges[2].member_summaries.len());
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql", "fixtures/activities.sql"))]
    async fn test_list_challenges_for_otheruser_nonadmin(pool: PgPool) {
        let err = crate::activities::list_challenges(
            State::from(&pool),
            State::from(&Config::load().unwrap()),
            find_user_login_by_name(&pool, "user2", "member").await,
            Some(find_person_id_by_name(&pool, "user1").await),
            None,
            None
        ).await.unwrap_err();
        assert_eq!(Custom(Status::Forbidden, "admin role required to view other user challenge totals".to_string()), err);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql", "fixtures/activities.sql"))]
    async fn test_get_challenge(pool: PgPool) {
        let challenge_id = find_challenge_by_name(&pool, "April 2025 Hikes").await;
        let user1 = find_person_id_by_name(&pool, "user1").await;
        let login = find_user_login_by_name(&pool, "admin", "admin").await;
        let challenge = crate::activities::get_challenge(State::from(&pool), login, challenge_id, Some(user1), None).await.unwrap();

        assert_eq!("April 2025 Hikes", challenge.name);
        assert_eq!("Hiking", challenge.activity_type.name);
        assert_eq!(300.0, challenge.total_for_person);
        assert_eq!(700.0, challenge.total_all);
        assert_eq!(2, challenge.member_summaries.len());
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql", "fixtures/activities.sql"))]
    async fn test_get_activity_self(pool: PgPool) {
        let login = find_user_login_by_name(&pool, "user1", "member").await;
        let challenge = find_challenge_by_name(&pool, "April 2025 Hikes").await;
        let activity_id: i64 = query_scalar("SELECT id FROM activity WHERE person_id = $1 AND challenge_id = $2 AND date = '2025-04-01'")
            .bind(login.uid).bind(challenge)
            .fetch_one(&pool)
            .await
            .unwrap();

        let activity = crate::activities::get_activity(State::from(&pool), login, activity_id).await.unwrap().0;
        assert_eq!(100.0, activity.amount);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql", "fixtures/activities.sql"))]
    async fn test_get_activity_other_admin(pool: PgPool) {
        let user1 = find_person_id_by_name(&pool, "user1").await;
        let challenge = find_challenge_by_name(&pool, "April 2025 Hikes").await;
        let activity_id: i64 = query_scalar("SELECT id FROM activity WHERE person_id = $1 AND challenge_id = $2 AND date = '2025-04-01'")
            .bind(user1).bind(challenge)
            .fetch_one(&pool)
            .await
            .unwrap();

        let login = find_user_login_by_name(&pool, "admin", "admin").await;
        let activity = crate::activities::get_activity(State::from(&pool), login, activity_id).await.unwrap().0;
        assert_eq!(100.0, activity.amount);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql", "fixtures/activities.sql"))]
    async fn test_get_activity_other_nonadmin(pool: PgPool) {
        let user1 = find_person_id_by_name(&pool, "user1").await;
        let challenge = find_challenge_by_name(&pool, "April 2025 Hikes").await;
        let activity_id: i64 = query_scalar("SELECT id FROM activity WHERE person_id = $1 AND challenge_id = $2 AND date = '2025-04-01'")
            .bind(user1).bind(challenge)
            .fetch_one(&pool)
            .await
            .unwrap();

        let login = find_user_login_by_name(&pool, "user2", "member").await;
        let err = crate::activities::get_activity(State::from(&pool), login, activity_id ).await.unwrap_err();
        assert_eq!(Custom(Status::Forbidden, "admin role required to view other user activities".to_string()), err);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql", "fixtures/activities.sql"))]
    async fn test_list_activities_self(pool: PgPool) {
        let challenge = find_challenge_by_name(&pool, "April 2025 Hikes").await;
        let user1 = find_person_id_by_name(&pool, "user1").await;

        let login = find_user_login_by_name(&pool, "user1", "member").await;
        let list = crate::activities::list_activities(State::from(&pool), login, Some(challenge), Some(user1), None, None, None).await.unwrap();
        assert_eq!(2, list.len());
        assert_eq!("Hiking", list[0].activity_type.name);
        assert_eq!("user1", list[0].person_name);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql", "fixtures/activities.sql"))]
    async fn test_list_activities_other_admin(pool: PgPool) {
        let challenge = find_challenge_by_name(&pool, "April 2025 Hikes").await;
        let user1 = find_person_id_by_name(&pool, "user1").await;
        let admin_login = find_user_login_by_name(&pool, "admin", "admin").await;

        let list = crate::activities::list_activities(State::from(&pool), admin_login, Some(challenge), Some(user1), None, None, None).await.unwrap();
        assert_eq!(2, list.len());
        assert_eq!("Hiking", list[0].activity_type.name);
        assert_eq!("user1", list[0].person_name);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql", "fixtures/activities.sql"))]
    async fn test_list_activities_allusers_admin(pool: PgPool) {
        let challenge = find_challenge_by_name(&pool, "April 2025 Hikes").await;
        let admin_login = find_user_login_by_name(&pool, "admin", "admin").await;

        let list = crate::activities::list_activities(State::from(&pool), admin_login, Some(challenge), None, None, None, None).await.unwrap();
        assert_eq!(3, list.len());
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql", "fixtures/activities.sql"))]
    async fn test_list_activities_other_nonadmin(pool: PgPool) {
        let challenge = find_challenge_by_name(&pool, "April 2025 Hikes").await;
        let user1 = find_person_id_by_name(&pool, "user1").await;
        let nonadmin_login = find_user_login_by_name(&pool, "user2", "member").await;

        let err = crate::activities::list_activities(State::from(&pool), nonadmin_login, Some(challenge), Some(user1), None, None, None).await.unwrap_err();
        assert_eq!(Custom(Status::Forbidden, "admin role required to view other user activities".to_string()), err);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql", "fixtures/activities.sql"))]
    async fn test_list_activities_allusers_nonadmin(pool: PgPool) {
        let challenge = find_challenge_by_name(&pool, "April 2025 Hikes").await;
        let nonadmin_login = find_user_login_by_name(&pool, "user2", "member").await;

        let err = crate::activities::list_activities(State::from(&pool), nonadmin_login, Some(challenge), None, None, None, None).await.unwrap_err();
        assert_eq!(Custom(Status::Forbidden, "admin role required to view other user activities".to_string()), err);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql"))]
    async fn test_create_activity_self(pool: PgPool) {
        let user = find_person_id_by_name(&pool, "user1").await;
        let activity = NewActivity {
            person_id: user,
            challenge_id: Some(find_challenge_by_name(&pool, "April 2025 Hikes").await),
            activity_type: None,
            date: NaiveDate::from_ymd_opt(2025, 4, 16).unwrap(),
            amount: 0.0
        };

        let login = find_user_login_by_name(&pool, "user1", "member").await;
        assert_eq!(0, count_activities(&pool).await);
        crate::activities::create_activity(State::from(&pool), login.clone(), Json(activity)).await.expect("Failed to create activity");
        let activities = list_activities(State::from(&pool), login, None, Some(user), None, None, None).await.unwrap().0;
        assert_eq!(1, activities.len());
        assert_eq!("Hiking", activities.get(0).unwrap().activity_type.name);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql"))]
    async fn test_create_activity_other_nonadmin(pool: PgPool) {
        let activity = NewActivity {
            person_id: find_person_id_by_name(&pool, "user1").await,
            challenge_id: Some(find_challenge_by_name(&pool, "April 2025 Hikes").await),
            activity_type: None,
            date: NaiveDate::from_ymd_opt(2025, 4, 16).unwrap(),
            amount: 0.0
        };

        let login = find_user_login_by_name(&pool, "user2", "member").await;
        assert_eq!(0, count_activities(&pool).await);
        let error = crate::activities::create_activity(State::from(&pool), login, Json(activity)).await.unwrap_err();
        assert_eq!(Custom(Status::Forbidden, "admin role required to create activities for other users".to_string()), error);
        assert_eq!(0, count_activities(&pool).await);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql"))]
    async fn test_create_activity_other_admin(pool: PgPool) {
        let activity = NewActivity {
            person_id: find_person_id_by_name(&pool, "user1").await,
            challenge_id: Some(find_challenge_by_name(&pool, "April 2025 Hikes").await),
            activity_type: None,
            date: NaiveDate::from_ymd_opt(2025, 4, 16).unwrap(),
            amount: 0.0
        };

        let login = find_user_login_by_name(&pool, "admin", "admin").await;
        assert_eq!(0, count_activities(&pool).await);
        crate::activities::create_activity(State::from(&pool), login, Json(activity)).await.unwrap();
        assert_eq!(1, count_activities(&pool).await);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql"))]
    async fn test_create_activity_without_challenge(pool: PgPool) {
        let activity = NewActivity {
            person_id: find_person_id_by_name(&pool, "user1").await,
            challenge_id: None,
            activity_type: Some(find_activity_type(&pool, "Cycling").await),
            date: NaiveDate::from_ymd_opt(2025, 4, 16).unwrap(),
            amount: 0.0
        };

        let login = find_user_login_by_name(&pool, "user1", "member").await;
        assert_eq!(0, count_activities(&pool).await);
        crate::activities::create_activity(State::from(&pool), login, Json(activity)).await.unwrap();
        assert_eq!(1, count_activities(&pool).await);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql"))]
    async fn test_create_activity_without_challenge_or_activity_type(pool: PgPool) {
        let activity = NewActivity {
            person_id: find_person_id_by_name(&pool, "user1").await,
            challenge_id: None,
            activity_type: None,
            date: NaiveDate::from_ymd_opt(2025, 4, 16).unwrap(),
            amount: 0.0
        };

        let login = find_user_login_by_name(&pool, "user1", "member").await;
        assert_eq!(0, count_activities(&pool).await);
        let err = crate::activities::create_activity(State::from(&pool), login, Json(activity)).await.unwrap_err();
        assert_eq!(Custom(Status::UnprocessableEntity, "either challenge_id or activity_type required".to_string()), err);
        assert_eq!(0, count_activities(&pool).await);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql"))]
    async fn test_create_activity_with_both_challenge_and_activity_type_agreed(pool: PgPool) {
        let user = find_person_id_by_name(&pool, "user1").await;
        let activity = NewActivity {
            person_id: user,
            challenge_id: Some(find_challenge_by_name(&pool, "April 2025 Hikes").await),
            activity_type: Some(find_activity_type(&pool, "Hiking").await),
            date: NaiveDate::from_ymd_opt(2025, 4, 16).unwrap(),
            amount: 0.0
        };

        let login = find_user_login_by_name(&pool, "user1", "member").await;
        assert_eq!(0, count_activities(&pool).await);
        crate::activities::create_activity(State::from(&pool), login.clone(), Json(activity)).await.unwrap();
        let activities = list_activities(State::from(&pool), login, None, Some(user), None, None, None).await.unwrap().0;
        assert_eq!(1, activities.len());
        assert_eq!("Hiking", activities.get(0).unwrap().activity_type.name);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql"))]
    async fn test_create_activity_with_both_challenge_and_activity_type_not_agreed(pool: PgPool) {
        let user = find_person_id_by_name(&pool, "user1").await;
        let activity = NewActivity {
            person_id: user,
            challenge_id: Some(find_challenge_by_name(&pool, "April 2025 Hikes").await),
            activity_type: Some(find_activity_type(&pool, "Cycling").await),
            date: NaiveDate::from_ymd_opt(2025, 4, 16).unwrap(),
            amount: 0.0
        };

        let login = find_user_login_by_name(&pool, "user1", "member").await;
        assert_eq!(0, count_activities(&pool).await);
        let err = crate::activities::create_activity(State::from(&pool), login, Json(activity)).await.unwrap_err();
        assert_eq!(Custom(Status::UnprocessableEntity, "activity_type clashes with challenge activity type".to_string()), err);
        assert_eq!(0, count_activities(&pool).await);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql", "fixtures/activities.sql"))]
    async fn test_delete_activity_self(pool: PgPool) {
        let challenge_id = find_challenge_by_name(&pool, "April 2025 Hikes").await;
        let user_id = find_person_id_by_name(&pool, "user1").await;

        let activities = Activity::query(&pool, Some(challenge_id), Some(user_id), None, None, None).await.unwrap();
        assert_eq!(2, activities.len());

        let login = find_user_login_by_name(&pool, "user1", "member").await;
        assert_eq!(7, count_activities(&pool).await);
        crate::activities::delete_activity(State::from(&pool), login, activities.first().unwrap().id).await.unwrap();
        assert_eq!(6, count_activities(&pool).await);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql", "fixtures/activities.sql"))]
    async fn test_delete_activity_other_admin(pool: PgPool) {
        let challenge_id = find_challenge_by_name(&pool, "April 2025 Hikes").await;
        let user_id = find_person_id_by_name(&pool, "user1").await;

        let activities = Activity::query(&pool, Some(challenge_id), Some(user_id), None, None, None).await.unwrap();
        assert_eq!(2, activities.len());

        let login = find_user_login_by_name(&pool, "admin", "admin").await;
        assert_eq!(7, count_activities(&pool).await);
        crate::activities::delete_activity(State::from(&pool), login, activities.first().unwrap().id).await.unwrap();
        assert_eq!(6, count_activities(&pool).await);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql", "fixtures/challenges.sql", "fixtures/activities.sql"))]
    async fn test_delete_activity_other_nonadmin(pool: PgPool) {
        let challenge_id = find_challenge_by_name(&pool, "April 2025 Hikes").await;
        let user_id = find_person_id_by_name(&pool, "user1").await;

        let activities = Activity::query(&pool, Some(challenge_id), Some(user_id), None, None, None).await.unwrap();
        assert_eq!(2, activities.len());

        let login = find_user_login_by_name(&pool, "user2", "member").await;
        assert_eq!(7, count_activities(&pool).await);
        let error = crate::activities::delete_activity(State::from(&pool), login, activities.first().unwrap().id).await.unwrap_err();
        assert_eq!(Custom(Status::NotFound, "activity not found or user not allowed to delete".to_string()), error);
        assert_eq!(7, count_activities(&pool).await);
    }

}
