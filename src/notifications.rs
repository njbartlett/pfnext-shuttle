#[cfg(test)]
use std::cell::RefCell;
use std::future::{Future, IntoFuture};

use mail_send::mail_builder::headers::address::Address;
use mail_send::mail_builder::MessageBuilder;
use mail_send::{Error, SmtpClientBuilder, Credentials};
use mail_send::smtp::message::{IntoMessage, Message};

use tokio::task::JoinHandle;

use crate::config::{AppEnv, Config};

pub fn send_email<'a>(
    config: &'a Config,
    app_env: &AppEnv,
    to_name: &'a str,
    to_email: &'a str,
    subject: &'a str,
    text: &'a str
) -> Result<(), mail_send::Error> {
    let mb = build_message(config, to_name, to_email, subject, text);
    internal_send_email_fire_forget(mb, config, app_env)
}

pub async fn send_email_async<'a>(
    config: &'a Config,
    app_env: &AppEnv,
    to_name: &'a str,
    to_email: &'a str,
    subject: &'a str,
    text: &'a str
) -> Result<(), mail_send::Error> {
    let mb = build_message(config, to_name, to_email, subject, text);
    internal_send_email_async(mb, config, app_env).await
}

pub fn send_admin_email(
    config: &Config,
    app_env: &AppEnv,
    subject: &str,
    text: &str
) -> Result<(), mail_send::Error> {
    let mb = MessageBuilder::new()
        .from(Address::new_address(Some(&config.email_sender_name), &config.email_sender_address))
        .reply_to(Address::new_address(Some(&config.email_replyto_name), &config.email_replyto_address))
        .to(config.email_admin_notifications.iter().map(|s| s.as_str()).collect::<Vec<&str>>())
        .subject(subject)
        .text_body(text);
    internal_send_email_fire_forget(mb, config, app_env)
}

fn build_message<'a>(
    config: &'a Config,
    to_name: &'a str,
    to_email: &'a str,
    subject: &'a str,
    text: &'a str
) -> MessageBuilder<'a> {
    MessageBuilder::new()
        .from(Address::new_address(Some(&config.email_sender_name), &config.email_sender_address))
        .reply_to(Address::new_address(Some(&config.email_replyto_name), &config.email_replyto_address))
        .to(Address::new_address(Some(to_name), to_email))
        .subject(subject)
        .text_body(text)
}

#[cfg(not(test))]
fn internal_send_email_fire_forget<'a>(
    mb: MessageBuilder<'a>,
    config: &Config,
    app_env: &AppEnv
) -> Result<(), mail_send::Error> {
    let message = mb.into_message()?;
    let client_builder = SmtpClientBuilder::new(config.smtp_host.to_string(), config.smtp_port)
        .implicit_tls(false)
        .credentials(Credentials::new(app_env.smtp_username.to_string(), app_env.smtp_password.to_string()));

    info!("Spawning task to send email");
    tokio::spawn(async move {
        internal_send_message(message, client_builder).await
    });
    info!("Spawned task to send email");

    Ok(())
}

#[cfg(not(test))]
async fn internal_send_email_async<'a>(
    mb: MessageBuilder<'a>,
    config: &Config,
    app_env: &AppEnv

) -> Result<(), mail_send::Error> {
    let message = mb.into_message()?;
    let client_builder = SmtpClientBuilder::new(config.smtp_host.to_string(), config.smtp_port)
        .implicit_tls(false)
        .credentials(Credentials::new(app_env.smtp_username.to_string(), app_env.smtp_password.to_string()));

    internal_send_message(message, client_builder).await?;
    Ok(())
}

#[cfg(test)]
thread_local! {
    static MOCK_SENT_MESSAGES: RefCell<Vec<(String, String, String)>> = RefCell::new(Vec::new());
}
#[cfg(test)]
pub fn get_sent_messages() -> Vec<(String, String, String)> {
    MOCK_SENT_MESSAGES.with_borrow(|vec| vec.clone())
}

#[cfg(test)]
fn internal_send_email_fire_forget<'a>(mb: MessageBuilder<'a>, _: &Config, _: &AppEnv) -> Result<(), Error> {
    save_to_sent_messages(mb);
    Ok(())
}
#[cfg(test)]
async fn internal_send_email_async<'a>(
    mb: MessageBuilder<'a>,
    config: &Config,
    app_env: &AppEnv

) -> Result<(), mail_send::Error> {
    save_to_sent_messages(mb);
    Ok(())
}

#[cfg(test)]
fn save_to_sent_messages(mb: MessageBuilder<'_>) {
    use mail_send::mail_builder::{headers::HeaderType, mime::BodyPart};

    let to_header = &mb.headers.iter()
        .filter(|h| h.0.eq("To"))
        .next()
        .unwrap().1;
    let to = match to_header {
        HeaderType::Address(a) => a.unwrap_address().email.to_string(),
        _ => "<<missing to address>>".to_string()
    };

    let subject_header = &mb.headers.iter()
        .filter(|h| h.0.eq("Subject"))
        .next()
        .unwrap().1;
    let subject = match subject_header {
        HeaderType::Text(t) => t.text.to_string(),
        _ => "<<missing subject>>".to_string()
    };
    
    let text = mb.text_body.as_ref().map(|m| &m.contents)
        .and_then(|bp| match bp {
            BodyPart::Text(t) => Some(t.to_string()),
            _ => None
        })
        .unwrap_or("<<missing text>>".to_string());
    MOCK_SENT_MESSAGES.with_borrow_mut(|vec| vec.push((to, subject, text)));

}

async fn internal_send_message<'a>(
    message: Message<'a>,
    client_builder: SmtpClientBuilder<String>
) -> Result<(), mail_send::Error> {
    info!("Connecting to SMTP server...");
    let mut client = client_builder
        .connect()
        .await?;
    info!("Connected to SMTP server, sending message...");
    client.send(message).await?;
    info!("Sent message over SMTP");
    
    Ok(())
}
