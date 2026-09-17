use rocket::{Route, State, http::Status, response::status::{Created, NoContent}, serde::json::Json};
use crate::apierror::ApiError;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, QueryBuilder, query, query_as, query_scalar};

use crate::{common::to_internal_server_err, loginsession::LoginSession, whereclause::{self, WhereClause}};

pub fn routes() -> Vec<Route> {
    routes![
        list_polls, create_poll, update_poll, delete_poll,
        post_vote, delete_vote
    ]
}

#[derive(Debug, FromRow, Serialize)]
pub struct Poll {
    id: i64,
    question: String,
    limit_per_person: i16,
    description: Option<String>,
    open: bool
}

impl Poll {
    const BASE_QUERY: &str = "SELECT id, question, limit_per_person, description, open FROM poll";
    async fn query_all(
        pool: &PgPool
    ) -> Result<Vec<Self>, sqlx::Error> {
        let mut qb = QueryBuilder::new(Self::BASE_QUERY);
        qb.push(" ORDER BY id ASC");
        qb.build_query_as()
            .fetch_all(pool)
            .await
    }

    async fn get_by_id(
        pool: &PgPool,
        poll_id: i64
    ) -> Result<Option<Self>, sqlx::Error> {
        let mut qb = QueryBuilder::new(Self::BASE_QUERY);
        qb.push(" WHERE id = ");
        qb.push_bind(poll_id);
        qb.push(" ORDER BY id ASC");
        
        qb.build_query_as()
            .fetch_optional(pool)
            .await
    }

    async fn create(
        pool: &PgPool,
        question: &str,
        limit_per_person: i16,
        description: Option<String>,
        open: bool
    ) -> Result<Self, sqlx::Error> {
        let id:i64 = query_scalar(
            "INSERT INTO poll (question, open, limit_per_person, description) VALUES ($1, $2, $3, $4) RETURNING id",
        )
        .bind(question)
        .bind(open)
        .bind(limit_per_person)
        .bind(description.clone()) // default open
        .fetch_one(pool)
        .await?;

        Ok(Self{
            id,
            question: question.to_string(),
            limit_per_person,
            description,
            open,
        })
    }

    /// Update all editable fields of a poll, returning the updated record or
    /// None if no poll with the given id exists.
    async fn update(
        pool: &PgPool,
        poll_id: i64,
        question: &str,
        limit_per_person: i16,
        description: Option<String>,
        open: bool
    ) -> Result<Option<Self>, sqlx::Error> {
        query_as(
            "UPDATE poll SET question = $1, limit_per_person = $2, description = $3, open = $4
                WHERE id = $5
                RETURNING id, question, limit_per_person, description, open")
            .bind(question)
            .bind(limit_per_person)
            .bind(description)
            .bind(open)
            .bind(poll_id)
            .fetch_optional(pool)
            .await
    }

    /// Delete a poll (and, via ON DELETE CASCADE, all of its votes). Returns
    /// true if a poll was deleted, false if no poll with the given id exists.
    async fn delete(pool: &PgPool, poll_id: i64) -> Result<bool, sqlx::Error> {
        let result = query("DELETE FROM poll WHERE id = $1")
            .bind(poll_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

/// Request body for creating or updating a poll.
#[derive(Debug, Deserialize, Serialize)]
struct NewPoll {
    question: String,
    limit_per_person: i16,
    description: Option<String>,
    #[serde(default = "default_open")]
    open: bool
}

fn default_open() -> bool { true }

impl NewPoll {
    fn validate(&self) -> Result<(), String> {
        if self.question.trim().is_empty() {
            return Err("poll question is required".to_string());
        }
        if self.limit_per_person < 1 {
            return Err("limit per person must be at least 1".to_string());
        }
        Ok(())
    }

    fn question(&self) -> &str {
        self.question.trim()
    }

    /// Trimmed description, with blank descriptions normalised to None
    fn description(&self) -> Option<String> {
        self.description.as_deref()
            .map(str::trim)
            .filter(|d| !d.is_empty())
            .map(str::to_string)
    }
}

#[derive(Debug, FromRow, Serialize)]
pub struct Vote {
    id: i64,
    poll_id: i64,
    person_id: i64,
    person_name: String,
    person_email: String,
    value: String
}

impl Vote {

    const BASE_QUERY: &str = "SELECT v.id, v.poll_id, v.person_id, v.value, p.name AS person_name, p.email AS person_email 
        FROM vote AS v
        JOIN person AS p ON v.person_id = p.id";

    pub async fn get_by_id(
        pool: &PgPool,
        vote_id: i64
    ) -> Result<Option<Self>, sqlx::Error> {
        QueryBuilder::new(Self::BASE_QUERY)
            .push(" WHERE v.id = ")
            .push_bind(vote_id)
            .build_query_as()
            .fetch_optional(pool)
            .await
    }

    pub async fn query(
        pool: &PgPool,
        poll_id: Option<i64>,
        person_id: Option<i64>
    ) -> Result<Vec<Self>, sqlx::Error> {
        let mut qb = QueryBuilder::new(Self::BASE_QUERY);
        WhereClause::init()
            .opt_append_to(&mut qb, "v.poll_id", whereclause::Operator::Equal, poll_id)
            .opt_append_to(&mut qb, "v.person_id", whereclause::Operator::Equal, person_id);
        qb.push(" ORDER BY v.id ASC");

        qb.build_query_as()
            .fetch_all(pool)
            .await
    }
     
    pub async fn create(pool: &PgPool, poll_id: i64, person_id: i64, value: &str) -> Result<Self, sqlx::Error> {
        let mut tx = pool.begin().await?;
        query("SELECT id from poll WHERE id = $1 FOR NO KEY UPDATE")
            .bind(poll_id)
            .fetch_one(&mut *tx)
            .await?;
        let vote_id: Option<i64> = query_scalar(
                "INSERT INTO vote (person_id, poll_id, value)
                    SELECT $1, $2, $3
                    FROM vote
                    WHERE poll_id = $2 AND person_id = $1
                    HAVING count(*) < (SELECT limit_per_person FROM poll WHERE id = $2)
                ON CONFLICT DO NOTHING
                RETURNING id")
            .bind(person_id)
            .bind(poll_id)
            .bind(value)
            .fetch_optional(&mut *tx)
            .await?;
        tx.commit().await?;

        match vote_id {
            Some(vote_id) => Self::get_by_id(pool, vote_id)
                .await?
                .ok_or_else(|| sqlx::Error::RowNotFound),
            None => Err(sqlx::Error::InvalidArgument("vote limit exceeded".to_string()))
        }
    }

    pub async fn delete(&self, pool: &PgPool) -> Result<(), sqlx::Error> {
        query("DELETE FROM vote WHERE id = $1")
            .bind(self.id)
            .execute(pool)
            .await?;
        Ok(())
    }

}

#[derive(Debug, Serialize)]
pub struct PollWithVotes {
    poll: Poll,
    votes: Vec<Vote>
}

impl PollWithVotes {
    async fn query(
        pool: &PgPool,
        poll_id: Option<i64>,
        person_id: Option<i64>,
        include_closed: bool,
    ) -> Result<Vec<Self>, sqlx::Error> {
        let polls = if let Some(poll_id) = poll_id {
            let poll = Poll::get_by_id(pool, poll_id).await?;
            match poll {
                Some(p) => vec![p],
                None => vec![]
            }
        } else {
            Poll::query_all(pool).await?
        };

        let mut result = Vec::new();
        for poll in polls {
            if !poll.open && !include_closed {
                continue;
            }
            let votes = Vote::query(pool, Some(poll.id), person_id).await?;
            result.push(Self {
                poll,
                votes
            });
        }
        Ok(result)
    }
}

#[get("/polls?<person_id>&<closed>")]
async fn list_polls(
    pool: &State<PgPool>,
    login: LoginSession,
    person_id: Option<i64>,
    closed: Option<bool>,
) -> Result<Json<Vec<PollWithVotes>>, ApiError> {
    if !login.is_admin() && Some(login.uid) != person_id {
        return Err(ApiError::new(Status::Forbidden, "admin role required to view other user votes".to_string()));
    }
    let closed = closed.unwrap_or(false);
    PollWithVotes::query(pool, None, person_id, closed)
        .await
        .map(Json)
        .map_err(to_internal_server_err)
}

fn require_admin(login: &LoginSession, action: &str) -> Result<(), ApiError> {
    if login.is_admin() {
        Ok(())
    } else {
        Err(ApiError::new(Status::Forbidden, format!("admin role required to {} polls", action)))
    }
}

#[post("/polls", data = "<new_poll>")]
async fn create_poll(
    pool: &State<PgPool>,
    login: LoginSession,
    new_poll: Json<NewPoll>
) -> Result<Created<Json<Poll>>, ApiError> {
    require_admin(&login, "create")?;
    new_poll.validate()
        .map_err(|e| ApiError::new(Status::BadRequest, e))?;

    let poll = Poll::create(pool, new_poll.question(), new_poll.limit_per_person, new_poll.description(), new_poll.open)
        .await
        .map_err(to_internal_server_err)?;
    info!("Created poll id {}", poll.id);
    Ok(Created::new(format!("/polls/{}", poll.id)).body(Json(poll)))
}

#[put("/polls/<poll_id>", data = "<new_poll>")]
async fn update_poll(
    pool: &State<PgPool>,
    login: LoginSession,
    poll_id: i64,
    new_poll: Json<NewPoll>
) -> Result<Json<Poll>, ApiError> {
    require_admin(&login, "update")?;
    new_poll.validate()
        .map_err(|e| ApiError::new(Status::BadRequest, e))?;

    let poll = Poll::update(pool, poll_id, new_poll.question(), new_poll.limit_per_person, new_poll.description(), new_poll.open)
        .await
        .map_err(to_internal_server_err)?
        .ok_or_else(|| ApiError::new(Status::NotFound, format!("poll id {} not found", poll_id)))?;
    info!("Updated poll id {}", poll.id);
    Ok(Json(poll))
}

#[delete("/polls/<poll_id>")]
async fn delete_poll(
    pool: &State<PgPool>,
    login: LoginSession,
    poll_id: i64
) -> Result<NoContent, ApiError> {
    require_admin(&login, "delete")?;

    let deleted = Poll::delete(pool, poll_id)
        .await
        .map_err(to_internal_server_err)?;
    if !deleted {
        return Err(ApiError::new(Status::NotFound, format!("poll id {} not found", poll_id)));
    }
    info!("Deleted poll id {}", poll_id);
    Ok(NoContent)
}

#[derive(Debug, Deserialize, Serialize)]
struct PostedVote {
    person_id: i64,
    poll_id: i64,
    value: String
}

#[post("/votes", data = "<vote>")]
async fn post_vote(
    pool: &State<PgPool>,
    login: LoginSession,
    vote: Json<PostedVote>
) -> Result<Json<Vote>, ApiError> {
    if !login.is_admin() && login.uid != vote.person_id {
        return Err(ApiError::new(Status::Forbidden, "admin role required to vote for other users".to_string()));
    }
    Vote::create(pool, vote.poll_id, vote.person_id, &vote.value)
        .await
        .map(Json)
        .map_err(to_internal_server_err)
}

#[delete("/votes/<vote_id>")]
async fn delete_vote(
    pool: &State<PgPool>,
    login: LoginSession,
    vote_id: i64
) -> Result<Json<Vote>, ApiError> {
    let vote = Vote::get_by_id(pool, vote_id)
        .await
        .map_err(to_internal_server_err)?
        .ok_or_else(|| ApiError::new(Status::NotFound, "vote not found".to_string()))?;
    if !login.is_admin() && login.uid != vote.person_id {
        return Err(ApiError::new(Status::Forbidden, "admin role required to delete other users' votes".to_string()));
    }

    vote.delete(pool).await
        .map_err(to_internal_server_err)?;
    return Ok(Json(vote));
}


#[cfg(test)]
mod tests {
    use rocket::{State, http::Status, serde::json::Json};
    use crate::apierror::ApiError;
    use sqlx::PgPool;

    use crate::{polls::{NewPoll, Poll, PollWithVotes, PostedVote, Vote}, testcommon::{find_person_id_by_name, find_user_login_by_name}};

    #[sqlx::test(fixtures("../schema.sql"))]
    async fn test_create_and_query_poll(pool: PgPool) {
        let created_poll = crate::polls::Poll::create(&pool, "What is your favorite colour?", 1, None, true).await.unwrap();
        assert_eq!(created_poll.question, "What is your favorite colour?");

        let queried_poll = crate::polls::Poll::get_by_id(&pool, created_poll.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(queried_poll.id, created_poll.id);
        assert_eq!(queried_poll.question, created_poll.question);

        crate::polls::Poll::query_all(&pool)
            .await
            .unwrap()
            .iter()
            .find(|p| p.id == created_poll.id)
            .expect("Created poll should be in the list of all polls");
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_query_all_polls_as_admin(pool: PgPool) {
        let poll = crate::polls::Poll::create(&pool, "What is your favorite colour?", 1, None, true).await.unwrap();
        let user1 = find_person_id_by_name(&pool, "user1").await;
        crate::polls::Vote::create(&pool, poll.id, user1, "Blue").await.unwrap();
        let user2 = find_person_id_by_name(&pool, "user2").await;
        crate::polls::Vote::create(&pool, poll.id, user2, "Green").await.unwrap();

        let login = find_user_login_by_name(&pool, "admin", "admin").await;
        let results = crate::polls::list_polls(
            &State::from(&pool),
            login,
            None,
            Some(false)
        ).await.expect("Admin SHOULD be able to query all polls").into_inner();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].poll.id, poll.id);

        assert_eq!(results[0].votes.len(), 2);
        assert_eq!(results[0].votes[0].value, "Blue");
        assert_eq!(results[0].votes[0].person_id, user1);
        assert_eq!(results[0].votes[0].person_name, "user1");

        assert_eq!(results[0].votes[1].value, "Green");
        assert_eq!(results[0].votes[1].person_id, user2);
        assert_eq!(results[0].votes[1].person_name, "user2");
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_query_all_polls_as_nonadmin(pool: PgPool) {
        crate::polls::Poll::create(&pool, "What is your favorite colour?", 1, None, true).await.unwrap();

        let login = find_user_login_by_name(&pool, "user1", "member").await;
        crate::polls::list_polls(
            &State::from(&pool),
            login,
            None,
            Some(false)
        ).await.expect_err("Non-admin SHOULD NOT be able to query all polls");
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_query_polls_for_member(pool: PgPool) {
        let poll = crate::polls::Poll::create(&pool, "What is your favorite colour?", 1, None, true).await.unwrap();
        let user1 = find_person_id_by_name(&pool, "user1").await;
        crate::polls::Vote::create(&pool, poll.id, user1, "Blue").await.unwrap();
        let user2 = find_person_id_by_name(&pool, "user2").await;
        crate::polls::Vote::create(&pool, poll.id, user2, "Green").await.unwrap();

        let login = find_user_login_by_name(&pool, "user1", "member").await;
        let results = crate::polls::list_polls(
            &State::from(&pool),
            login,
            Some(user1),
            Some(false)
        ).await.expect("Non-admin SHOULD be able to query poll for self").into_inner();
        assert_eq!(results[0].votes.len(), 1);
        assert_eq!(results[0].votes[0].person_id, user1);
        assert_eq!(results[0].votes[0].value, "Blue");
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_post_vote_as_member(pool: PgPool) {
        let poll = crate::polls::Poll::create(&pool, "What is your favorite colour?", 1, None, true).await.unwrap();
        let login = find_user_login_by_name(&pool, "user1", "member").await;
        crate::polls::post_vote(
            &State::from(&pool),
            login.clone(),
            Json(PostedVote {
                person_id: login.uid,
                poll_id: poll.id,
                value: "Blue".to_string()
            })
        ).await.expect("Non-admin SHOULD be able to post vote for self");

        let polls = PollWithVotes::query(
            &pool,
            Some(poll.id),
            None,
            false
        ).await.expect("Query after posting vote should succeed");
        assert_eq!(polls.len(), 1);
        assert_eq!(polls[0].poll.id, poll.id);
        assert_eq!(polls[0].votes.len(), 1);
        assert_eq!(polls[0].votes[0].person_id, login.uid);
        assert_eq!(polls[0].votes[0].value, "Blue");
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_post_vote_as_member_wrong_person(pool: PgPool) {
        let poll = crate::polls::Poll::create(&pool, "What is your favorite colour?", 1, None, true).await.unwrap();
        let login = find_user_login_by_name(&pool, "user1", "member").await;
        let user2 = find_person_id_by_name(&pool, "user2").await;
        crate::polls::post_vote(
            &State::from(&pool),
            login.clone(),
            Json(PostedVote {
                person_id: user2,
                poll_id: poll.id,
                value: "Blue".to_string()
            })
        ).await.expect_err("Non-admin SHOULD NOT be able to post vote for others");


        let polls = PollWithVotes::query(
            &pool,
            Some(poll.id),
            None,
            false
        ).await.expect("Query after posting vote should succeed");
        assert_eq!(polls.len(), 1);
        assert_eq!(polls[0].poll.id, poll.id);
        assert_eq!(polls[0].votes.len(), 0);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_post_vote_as_admin_other_person(pool: PgPool) {
        let poll = crate::polls::Poll::create(&pool, "What is your favorite colour?", 1, None, true).await.unwrap();
        let login = find_user_login_by_name(&pool, "admin", "admin").await;
        let user2 = find_person_id_by_name(&pool, "user2").await;
        crate::polls::post_vote(
            &State::from(&pool),
            login.clone(),
            Json(PostedVote {
                person_id: user2,
                poll_id: poll.id,
                value: "Blue".to_string()
            })
        ).await.expect("Admin SHOULD be able to post vote for other");

        let polls = PollWithVotes::query(
            &pool,
            Some(poll.id),
            None,
            false
        ).await.expect("Query after posting vote should succeed");
        assert_eq!(polls.len(), 1);
        assert_eq!(polls[0].poll.id, poll.id);
        assert_eq!(polls[0].votes.len(), 1);
        assert_eq!(polls[0].votes[0].person_id, user2);
        assert_eq!(polls[0].votes[0].value, "Blue");
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_post_multiple_votes(pool: PgPool) {
        let poll = crate::polls::Poll::create(&pool, "What is your favorite colour?", 2, None, true).await.unwrap();
        let login = find_user_login_by_name(&pool, "user1", "member").await;
        crate::polls::post_vote(
            &State::from(&pool),
            login.clone(),
            Json(PostedVote {
                person_id: login.uid,
                poll_id: poll.id,
                value: "Blue".to_string()
            })
        ).await.expect("Non-admin SHOULD be able to post vote for self");
        let votes_after_first_post = Vote::query(&pool, Some(poll.id), Some(login.uid)).await.expect("Query after posting vote should succeed");
        assert_eq!(votes_after_first_post.len(), 1);
        assert_eq!(votes_after_first_post[0].value, "Blue");

        crate::polls::post_vote(
            &State::from(&pool),
            login.clone(),
            Json(PostedVote {
                person_id: login.uid,
                poll_id: poll.id,
                value: "No wait, green!".to_string()
            })
        ).await.expect("Non-admin SHOULD be able to post vote for self");
        let votes_after_second_post = Vote::query(&pool, Some(poll.id), Some(login.uid)).await.expect("Query after posting vote should succeed");
        assert_eq!(votes_after_second_post.len(), 2);
        assert_eq!(votes_after_second_post[0].value, "Blue");
        assert_eq!(votes_after_second_post[1].value, "No wait, green!");
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_post_multiple_votes_over_limit(pool: PgPool) {
        let poll = crate::polls::Poll::create(&pool, "What is your favorite colour?", 2, None, true).await.unwrap();
        let login = find_user_login_by_name(&pool, "user1", "member").await;
        crate::polls::post_vote(
            &State::from(&pool),
            login.clone(),
            Json(PostedVote {
                person_id: login.uid,
                poll_id: poll.id,
                value: "Blue".to_string()
            })
        ).await.expect("SHOULD be able to post first vote when limit is 2");
        let votes_after_first_post = Vote::query(&pool, Some(poll.id), Some(login.uid)).await.expect("Query after posting vote should succeed");
        assert_eq!(votes_after_first_post.len(), 1);
        assert_eq!(votes_after_first_post[0].value, "Blue");

        crate::polls::post_vote(
            &State::from(&pool),
            login.clone(),
            Json(PostedVote {
                person_id: login.uid,
                poll_id: poll.id,
                value: "No wait, green!".to_string()
            })
        ).await.expect("SHOULD be able to post second vote when limit is 2");
        let votes_after_second_post = Vote::query(&pool, Some(poll.id), Some(login.uid)).await.expect("Query after posting vote should succeed");
        assert_eq!(votes_after_second_post.len(), 2);
        assert_eq!(votes_after_second_post[0].value, "Blue");
        assert_eq!(votes_after_second_post[1].value, "No wait, green!");

        let third_vote_err = crate::polls::post_vote(
            &State::from(&pool),
            login.clone(),
            Json(PostedVote {
                person_id: login.uid,
                poll_id: poll.id,
                value: "Also yello".to_string()
            })
        ).await.expect_err("SHOULD NOT be able to post third vote when limit is 2");
        assert_eq!(third_vote_err, ApiError::new(Status::InternalServerError, "vote limit exceeded".to_string()));
        let votes_after_second_post = Vote::query(&pool, Some(poll.id), Some(login.uid)).await.expect("Query after posting vote should succeed");
        assert_eq!(votes_after_second_post.len(), 2);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_query_polls_open_and_closed(pool: PgPool) {
        let poll1 = crate::polls::Poll::create(&pool, "What is your favorite colour?", 1, None, true).await.unwrap();
        let poll2 = crate::polls::Poll::create(&pool, "What's the air-speed velocity of an unladen swallow?", 1, None, false).await.unwrap();

        let user1 = find_person_id_by_name(&pool, "user1").await;
        let user2 = find_person_id_by_name(&pool, "user2").await;

        crate::polls::Vote::create(&pool, poll1.id, user1, "Blue").await.unwrap();
        crate::polls::Vote::create(&pool, poll1.id, user2, "Green").await.unwrap();

        crate::polls::Vote::create(&pool, poll2.id, user1, "I don't know that!").await.unwrap();
        crate::polls::Vote::create(&pool, poll2.id, user2, "What do you mean, an African or European swallow?").await.unwrap();

        let login = find_user_login_by_name(&pool, "user1", "member").await;

        // Include closed polls in query
        let results = crate::polls::list_polls(
            &State::from(&pool),
            login.clone(),
            Some(user1),
            Some(true)
        ).await.unwrap().into_inner();
        assert_eq!(2, results.len());
        assert_eq!(results[0].votes.len(), 1);
        assert_eq!(results[0].votes[0].person_id, user1);
        assert_eq!(results[0].votes[0].value, "Blue");
        assert_eq!(results[1].votes.len(), 1);
        assert_eq!(results[1].votes[0].person_id, user1);
        assert_eq!(results[1].votes[0].value, "I don't know that!");

        // Exclude closed polls in query
        let results = crate::polls::list_polls(
            &State::from(&pool),
            login.clone(),
            Some(user1),
            Some(false)
        ).await.unwrap().into_inner();
        assert_eq!(1, results.len());
        assert_eq!(results[0].votes.len(), 1);
        assert_eq!(results[0].votes[0].person_id, user1);
        assert_eq!(results[0].votes[0].value, "Blue");

        // Default = exclude closed polls in query
        let results = crate::polls::list_polls(
            &State::from(&pool),
            login.clone(),
            Some(user1),
            None
        ).await.unwrap().into_inner();
        assert_eq!(1, results.len());
        assert_eq!(results[0].votes.len(), 1);
        assert_eq!(results[0].votes[0].person_id, user1);
        assert_eq!(results[0].votes[0].value, "Blue");
    }

    fn new_poll(question: &str, limit_per_person: i16, description: Option<&str>, open: bool) -> Json<NewPoll> {
        Json(NewPoll {
            question: question.to_string(),
            limit_per_person,
            description: description.map(str::to_string),
            open
        })
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_create_poll_as_admin(pool: PgPool) {
        let login = find_user_login_by_name(&pool, "admin", "admin").await;
        crate::polls::create_poll(
            &State::from(&pool),
            login,
            new_poll("  What is your favorite colour?  ", 2, Some("   "), false)
        ).await.expect("Admin SHOULD be able to create a poll");

        let polls = Poll::query_all(&pool).await.unwrap();
        assert_eq!(polls.len(), 1);
        // Question is trimmed and blank description is normalised to None
        assert_eq!(polls[0].question, "What is your favorite colour?");
        assert_eq!(polls[0].limit_per_person, 2);
        assert_eq!(polls[0].description, None);
        assert_eq!(polls[0].open, false);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_create_poll_as_member_forbidden(pool: PgPool) {
        let login = find_user_login_by_name(&pool, "user1", "member").await;
        let err = crate::polls::create_poll(
            &State::from(&pool),
            login,
            new_poll("What is your favorite colour?", 1, None, true)
        ).await.expect_err("Non-admin SHOULD NOT be able to create a poll");
        assert_eq!(err.0, Status::Forbidden);
        assert_eq!(Poll::query_all(&pool).await.unwrap().len(), 0);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_create_poll_validation(pool: PgPool) {
        let login = find_user_login_by_name(&pool, "admin", "admin").await;

        let err = crate::polls::create_poll(
            &State::from(&pool),
            login.clone(),
            new_poll("   ", 1, None, true)
        ).await.expect_err("Blank question SHOULD be rejected");
        assert_eq!(err.0, Status::BadRequest);

        let err = crate::polls::create_poll(
            &State::from(&pool),
            login.clone(),
            new_poll("What is your favorite colour?", 0, None, true)
        ).await.expect_err("Zero vote limit SHOULD be rejected");
        assert_eq!(err.0, Status::BadRequest);

        assert_eq!(Poll::query_all(&pool).await.unwrap().len(), 0);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_update_poll_as_admin(pool: PgPool) {
        let poll = Poll::create(&pool, "What is your favorite colour?", 1, None, true).await.unwrap();
        let login = find_user_login_by_name(&pool, "admin", "admin").await;

        // Edit the question and close the poll
        let updated = crate::polls::update_poll(
            &State::from(&pool),
            login.clone(),
            poll.id,
            new_poll("What is your favourite colour?", 3, Some("Pick up to three"), false)
        ).await.expect("Admin SHOULD be able to update a poll").into_inner();
        assert_eq!(updated.id, poll.id);
        assert_eq!(updated.question, "What is your favourite colour?");
        assert_eq!(updated.limit_per_person, 3);
        assert_eq!(updated.description, Some("Pick up to three".to_string()));
        assert_eq!(updated.open, false);

        let stored = Poll::get_by_id(&pool, poll.id).await.unwrap().unwrap();
        assert_eq!(stored.question, "What is your favourite colour?");
        assert_eq!(stored.limit_per_person, 3);
        assert_eq!(stored.description, Some("Pick up to three".to_string()));
        assert_eq!(stored.open, false);

        // Reopen the poll
        let reopened = crate::polls::update_poll(
            &State::from(&pool),
            login.clone(),
            poll.id,
            new_poll("What is your favourite colour?", 3, Some("Pick up to three"), true)
        ).await.expect("Admin SHOULD be able to reopen a poll").into_inner();
        assert_eq!(reopened.open, true);
        assert_eq!(Poll::get_by_id(&pool, poll.id).await.unwrap().unwrap().open, true);

        // Unknown poll id
        let err = crate::polls::update_poll(
            &State::from(&pool),
            login.clone(),
            poll.id + 1000,
            new_poll("Does not exist", 1, None, true)
        ).await.expect_err("Updating an unknown poll SHOULD fail");
        assert_eq!(err.0, Status::NotFound);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_update_poll_as_member_forbidden(pool: PgPool) {
        let poll = Poll::create(&pool, "What is your favorite colour?", 1, None, true).await.unwrap();
        let login = find_user_login_by_name(&pool, "user1", "member").await;
        let err = crate::polls::update_poll(
            &State::from(&pool),
            login,
            poll.id,
            new_poll("Hacked", 1, None, false)
        ).await.expect_err("Non-admin SHOULD NOT be able to update a poll");
        assert_eq!(err.0, Status::Forbidden);

        let stored = Poll::get_by_id(&pool, poll.id).await.unwrap().unwrap();
        assert_eq!(stored.question, "What is your favorite colour?");
        assert_eq!(stored.open, true);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_delete_poll_as_admin_cascades_votes(pool: PgPool) {
        let poll = Poll::create(&pool, "What is your favorite colour?", 1, None, true).await.unwrap();
        let other_poll = Poll::create(&pool, "Unrelated poll", 1, None, true).await.unwrap();
        let user1 = find_person_id_by_name(&pool, "user1").await;
        let user2 = find_person_id_by_name(&pool, "user2").await;
        Vote::create(&pool, poll.id, user1, "Blue").await.unwrap();
        Vote::create(&pool, poll.id, user2, "Green").await.unwrap();
        Vote::create(&pool, other_poll.id, user1, "Keep me").await.unwrap();

        let login = find_user_login_by_name(&pool, "admin", "admin").await;
        crate::polls::delete_poll(
            &State::from(&pool),
            login.clone(),
            poll.id
        ).await.expect("Admin SHOULD be able to delete a poll");

        assert!(Poll::get_by_id(&pool, poll.id).await.unwrap().is_none());
        assert_eq!(Vote::query(&pool, Some(poll.id), None).await.unwrap().len(), 0, "votes on the deleted poll should be removed");
        assert_eq!(Vote::query(&pool, Some(other_poll.id), None).await.unwrap().len(), 1, "votes on other polls should be untouched");

        // Deleting again is a 404
        let err = crate::polls::delete_poll(
            &State::from(&pool),
            login,
            poll.id
        ).await.expect_err("Deleting an unknown poll SHOULD fail");
        assert_eq!(err.0, Status::NotFound);
    }

    #[sqlx::test(fixtures("../schema.sql", "fixtures/users.sql"))]
    async fn test_delete_poll_as_member_forbidden(pool: PgPool) {
        let poll = Poll::create(&pool, "What is your favorite colour?", 1, None, true).await.unwrap();
        let login = find_user_login_by_name(&pool, "user1", "member").await;
        let err = crate::polls::delete_poll(
            &State::from(&pool),
            login,
            poll.id
        ).await.expect_err("Non-admin SHOULD NOT be able to delete a poll");
        assert_eq!(err.0, Status::Forbidden);
        assert!(Poll::get_by_id(&pool, poll.id).await.unwrap().is_some());
    }

}