use rocket::{Route, State, http::Status, response::status::Custom, serde::json::Json};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, QueryBuilder, query, query_as, query_scalar};

use crate::{common::to_internal_server_err, loginsession::LoginSession, whereclause::{self, WhereClause}};

pub fn routes() -> Vec<Route> {
    routes![
        list_polls, post_vote, delete_vote
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
        person_id: Option<i64>
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
            let votes = Vote::query(pool, Some(poll.id), person_id).await?;
            result.push(Self {
                poll,
                votes
            });
        }
        Ok(result)
    }
}

#[get("/polls?<person_id>")]
async fn list_polls(
    pool: &State<PgPool>,
    login: LoginSession,
    person_id: Option<i64>,
) -> Result<Json<Vec<PollWithVotes>>, Custom<String>> {
    if !login.is_admin() && Some(login.uid) != person_id {
        return Err(Custom(Status::Forbidden, "admin role required to view other user votes".to_string()));
    }
    PollWithVotes::query(pool, None, person_id)
        .await
        .map(Json)
        .map_err(to_internal_server_err)
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
) -> Result<Json<Vote>, Custom<String>> {
    if !login.is_admin() && login.uid != vote.person_id {
        return Err(Custom(Status::Forbidden, "admin role required to vote for other users".to_string()));
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
) -> Result<Json<Vote>, Custom<String>> {
    let vote = Vote::get_by_id(pool, vote_id)
        .await
        .map_err(to_internal_server_err)?
        .ok_or_else(|| Custom(Status::NotFound, "vote not found".to_string()))?;
    if !login.is_admin() && login.uid != vote.person_id {
        return Err(Custom(Status::Forbidden, "admin role required to delete other users' votes".to_string()));
    }

    vote.delete(pool).await
        .map_err(to_internal_server_err)?;
    return Ok(Json(vote));
}


#[cfg(test)]
mod tests {
    use rocket::{State, http::Status, response::status::Custom, serde::json::Json};
    use sqlx::PgPool;

    use crate::{polls::{PollWithVotes, PostedVote, Vote}, testcommon::{find_person_id_by_name, find_user_login_by_name}};

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
            None
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
            None
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
            Some(user1)
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
            None
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
            None
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
            None
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
        assert_eq!(third_vote_err, Custom(Status::InternalServerError, "vote limit exceeded".to_string()));
        let votes_after_second_post = Vote::query(&pool, Some(poll.id), Some(login.uid)).await.expect("Query after posting vote should succeed");
        assert_eq!(votes_after_second_post.len(), 2);
    }

}