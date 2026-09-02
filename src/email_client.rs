use crate::domain::SubscriberEmail;
use reqwest::Client;
use secrecy::ExposeSecret;
use secrecy::SecretString;

pub struct EmailClient {
    http_client: Client,
    base_url: String,
    sender: SubscriberEmail,
    api_token: SecretString,
}

// Lifetime parameters always start with an apostrophe, `'`
#[derive(serde::Serialize)]
struct SendEmailRequest<'a> {
    from: EmailAddress<'a>,
    to: Vec<EmailAddress<'a>>,
    subject: &'a str,
    html: &'a str,
    text: &'a str,
}

#[derive(serde::Serialize)]
struct EmailAddress<'a> {
    email: &'a str,
}

impl EmailClient {
    pub fn new(
        base_url: String,
        sender: SubscriberEmail,
        api_token: SecretString,
        timeout: std::time::Duration,
    ) -> Self {
        let http_client = Client::builder().timeout(timeout).build().unwrap();
        Self {
            http_client,
            base_url,
            sender,
            api_token,
        }
    }

    pub async fn send_email(
        &self,
        recipient: SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str,
    ) -> Result<(), reqwest::Error> {
        let url = &self.base_url;

        let request_body = SendEmailRequest {
            from: EmailAddress {
                email: self.sender.as_ref(),
            },
            to: vec![EmailAddress {
                email: recipient.as_ref(),
            }],
            subject,
            html: html_content,
            text: text_content,
        };

        self.http_client
            .post(url)
            .bearer_auth(self.api_token.expose_secret())
            .json(&request_body)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use claims::assert_err;
    use claims::assert_ok;
    use fake::faker::internet::en::SafeEmail;
    use fake::faker::lorem::en::{Paragraph, Sentence};
    use fake::{Fake, Faker};
    use secrecy::SecretString;
    use wiremock::matchers::any;
    use wiremock::matchers::{header, method};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    struct SendEmailBodyMatcher;

    /// Generate a random email subject
    fn subject() -> String {
        Sentence(1..2).fake()
    }

    /// Generate a random email content
    fn content() -> String {
        Paragraph(1..10).fake()
    }

    /// Generate a random subscriber email
    fn subscriber_email() -> SubscriberEmail {
        SubscriberEmail::parse(SafeEmail().fake()).unwrap()
    }

    impl wiremock::Match for SendEmailBodyMatcher {
        fn matches(&self, request: &Request) -> bool {
            let result: Result<serde_json::Value, _> = serde_json::from_slice(&request.body);
            if let Ok(body) = result {
                // Check Mailtrap's schema fields
                body.get("from").is_some()
                    && body.get("to").is_some()
                    && body.get("subject").is_some()
                    && body.get("html").is_some()
                    && body.get("text").is_some()
            } else {
                false
            }
        }
    }

    #[tokio::test]
    async fn send_email_sends_the_expected_request() {
        // Arrange
        let mock_server = MockServer::start().await;
        let sender = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let api_token = SecretString::from(Faker.fake::<String>());
        let email_client = EmailClient::new(
            mock_server.uri(),
            sender,
            api_token.clone(),
            std::time::Duration::from_millis(200),
        );

        Mock::given(header(
            "Authorization",
            format!("Bearer {}", api_token.expose_secret()),
        ))
        .and(header("Content-Type", "application/json"))
        .and(method("POST"))
        .and(SendEmailBodyMatcher)
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&mock_server)
        .await;

        // Act
        let _ = email_client
            .send_email(subscriber_email(), &subject(), &content(), &content())
            .await;

        // Assert
        // Mock expectations are verified automatically on drop
    }

    #[tokio::test]
    async fn send_email_succeeds_if_the_server_returns_200() {
        // Arrange
        let mock_server = MockServer::start().await;
        let sender = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let api_token = SecretString::from(Faker.fake::<String>());
        let email_client = EmailClient::new(
            mock_server.uri(),
            sender,
            api_token.clone(),
            std::time::Duration::from_millis(200),
        );

        // We do not copy in all the matchers we have in the other test.
        // The purpose of this test is not to assert on the request we
        // are sending out!
        // We add the bare minimum needed to trigger the path we want
        // to test in `send_email`.
        Mock::given(any())
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        // Act
        let outcome = email_client
            .send_email(subscriber_email(), &subject(), &content(), &content())
            .await;

        // Assert
        assert_ok!(outcome);
    }

    #[tokio::test]
    async fn send_email_fails_if_the_server_returns_500() {
        // Arrange
        let mock_server = MockServer::start().await;
        let sender = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let api_token = SecretString::from(Faker.fake::<String>());
        let email_client = EmailClient::new(
            mock_server.uri(),
            sender,
            api_token.clone(),
            std::time::Duration::from_millis(200),
        );

        Mock::given(any())
            // Not a 200 anymore!
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&mock_server)
            .await;

        // Act
        let outcome = email_client
            .send_email(subscriber_email(), &subject(), &content(), &content())
            .await;

        // Assert
        assert_err!(outcome);
    }

    #[tokio::test]
    async fn send_email_times_out_if_the_server_takes_too_long() {
        // Arrange
        let mock_server = MockServer::start().await;
        let sender = SubscriberEmail::parse(SafeEmail().fake()).unwrap();
        let api_token = SecretString::from(Faker.fake::<String>());
        let email_client = EmailClient::new(
            mock_server.uri(),
            sender,
            api_token.clone(),
            std::time::Duration::from_millis(200),
        );

        let response = ResponseTemplate::new(200)
            // 3 minutes!
            .set_delay(std::time::Duration::from_secs(180));
        Mock::given(any())
            .respond_with(response)
            .expect(1)
            .mount(&mock_server)
            .await;

        // Act
        let outcome = email_client
            .send_email(subscriber_email(), &subject(), &content(), &content())
            .await;

        // Assert
        assert_err!(outcome);
    }
}
