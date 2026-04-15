use std::time::Duration;

use reqwest::StatusCode;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

use crate::cli::PostStatus;
use crate::error::CommandError;

const ACCOUNT_QUERY: &str = r#"
query AccountOrganizations {
  account {
    organizations {
      id
      name
      ownerEmail
      channelCount
      limits {
        scheduledPosts
        scheduledStoriesPerChannel
        ideas
        tags
      }
    }
  }
}
"#;

const CHANNELS_QUERY: &str = r#"
query OrganizationChannels($input: ChannelsInput!) {
  channels(input: $input) {
    id
    name
    displayName
    service
    type
    organizationId
    isLocked
    isDisconnected
    isQueuePaused
    timezone
    products
    externalLink
  }
}
"#;

const CHANNEL_QUERY: &str = r#"
query ChannelById($input: ChannelInput!) {
  channel(input: $input) {
    id
    name
    displayName
    service
    type
    organizationId
    isLocked
    isDisconnected
    isQueuePaused
    timezone
    products
    externalLink
  }
}
"#;

const POSTS_QUERY: &str = r#"
query ListPosts($input: PostsInput!, $first: Int, $after: String) {
  posts(input: $input, first: $first, after: $after) {
    edges {
      cursor
      node {
        ...PostFields
      }
    }
    pageInfo {
      hasNextPage
      endCursor
    }
  }
}

fragment PostFields on Post {
  id
  status
  via
  schedulingType
  shareMode
  createdAt
  updatedAt
  dueAt
  sentAt
  text
  externalLink
  channelId
  channelService
  assets {
    __typename
    thumbnail
    mimeType
    source
    ... on ImageAsset {
      image {
        altText
        width
        height
      }
    }
  }
}
"#;

const POST_QUERY: &str = r#"
query GetPost($input: PostInput!) {
  post(input: $input) {
    ...PostFields
  }
}

fragment PostFields on Post {
  id
  status
  via
  schedulingType
  shareMode
  createdAt
  updatedAt
  dueAt
  sentAt
  text
  externalLink
  channelId
  channelService
  assets {
    __typename
    thumbnail
    mimeType
    source
    ... on ImageAsset {
      image {
        altText
        width
        height
      }
    }
  }
}
"#;

const DAILY_POSTING_LIMITS_QUERY: &str = r#"
query DailyPostingLimits($input: DailyPostingLimitsInput!) {
  dailyPostingLimits(input: $input) {
    channelId
    sent
    scheduled
    limit
    isAtLimit
  }
}
"#;

const CREATE_POST_MUTATION: &str = r#"
mutation CreatePost($input: CreatePostInput!) {
  createPost(input: $input) {
    __typename
    ... on PostActionSuccess {
      post {
        ...PostFields
      }
    }
    ... on MutationError {
      message
    }
  }
}

fragment PostFields on Post {
  id
  status
  via
  schedulingType
  shareMode
  createdAt
  updatedAt
  dueAt
  sentAt
  text
  externalLink
  channelId
  channelService
  assets {
    __typename
    thumbnail
    mimeType
    source
    ... on ImageAsset {
      image {
        altText
        width
        height
      }
    }
  }
}
"#;

const DELETE_POST_MUTATION: &str = r#"
mutation DeletePost($input: DeletePostInput!) {
  deletePost(input: $input) {
    __typename
    ... on DeletePostSuccess {
      id
    }
    ... on MutationError {
      message
    }
  }
}
"#;

#[derive(Debug)]
pub struct BufferClient {
    http: Client,
    base_url: String,
    token: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BufferWarning {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ApiResponse<T> {
    pub data: T,
    pub warnings: Vec<BufferWarning>,
}

impl<T> ApiResponse<T> {
    fn new(data: T, warnings: Vec<BufferWarning>) -> Self {
        Self { data, warnings }
    }

    fn map<U>(self, transform: impl FnOnce(T) -> U) -> ApiResponse<U> {
        ApiResponse {
            data: transform(self.data),
            warnings: self.warnings,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Organization {
    pub id: String,
    pub name: String,
    pub owner_email: Option<String>,
    pub channel_count: Option<u64>,
    pub limits: Option<OrganizationLimits>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationLimits {
    pub scheduled_posts: Option<u64>,
    pub scheduled_stories_per_channel: Option<u64>,
    pub ideas: Option<u64>,
    pub tags: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub display_name: Option<String>,
    pub service: String,
    #[serde(rename = "type")]
    pub channel_type: Option<String>,
    pub organization_id: String,
    #[serde(default)]
    pub is_locked: bool,
    #[serde(default)]
    pub is_disconnected: bool,
    #[serde(default)]
    pub is_queue_paused: bool,
    pub timezone: Option<String>,
    #[serde(default)]
    pub products: Vec<String>,
    pub external_link: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Post {
    pub id: String,
    pub status: String,
    pub via: Option<String>,
    pub scheduling_type: Option<String>,
    pub share_mode: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub due_at: Option<String>,
    pub sent_at: Option<String>,
    #[serde(default)]
    pub text: String,
    pub external_link: Option<String>,
    pub channel_id: String,
    pub channel_service: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub assets: Vec<PostAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostAsset {
    #[serde(rename = "__typename")]
    pub kind: String,
    pub thumbnail: Option<String>,
    pub mime_type: Option<String>,
    pub source: Option<String>,
    pub image: Option<PostImage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostImage {
    pub alt_text: Option<String>,
    pub width: Option<u64>,
    pub height: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostListResponse {
    pub posts: Vec<Post>,
    pub page_info: PageInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyPostingLimitStatus {
    pub channel_id: String,
    pub sent: u64,
    pub scheduled: u64,
    pub limit: Option<u64>,
    pub is_at_limit: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ListPostsOptions {
    pub organization_id: String,
    pub channel_ids: Vec<String>,
    pub status: Option<PostStatus>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: usize,
    pub cursor: Option<String>,
}

impl BufferClient {
    pub fn new(base_url: String, token: String, timeout_ms: u64) -> Result<Self, CommandError> {
        let http = Client::builder()
            .timeout(Duration::from_millis(timeout_ms.max(1)))
            .build()
            .map_err(|error| {
                CommandError::blocked(
                    "CLIENT_INIT_ERROR",
                    format!("failed to initialize HTTP client: {error}"),
                    "Check the configured Buffer API base URL and retry",
                )
            })?;

        Ok(Self {
            http,
            base_url,
            token,
        })
    }

    pub fn list_organizations(&self) -> Result<ApiResponse<Vec<Organization>>, CommandError> {
        let response: ApiResponse<AccountQueryResponse> = self.graphql(ACCOUNT_QUERY, json!({}))?;
        Ok(response.map(|payload| payload.account.organizations))
    }

    pub fn list_channels(
        &self,
        organization_id: &str,
    ) -> Result<ApiResponse<Vec<Channel>>, CommandError> {
        let response: ApiResponse<ChannelsQueryResponse> = self.graphql(
            CHANNELS_QUERY,
            json!({
                "input": {
                    "organizationId": organization_id,
                }
            }),
        )?;
        Ok(response.map(|payload| payload.channels))
    }

    pub fn get_channel(
        &self,
        channel_id: &str,
    ) -> Result<ApiResponse<Option<Channel>>, CommandError> {
        let response: ApiResponse<ChannelQueryResponse> = self.graphql(
            CHANNEL_QUERY,
            json!({
                "input": {
                    "id": channel_id,
                }
            }),
        )?;
        Ok(response.map(|payload| payload.channel))
    }

    pub fn list_posts(
        &self,
        options: &ListPostsOptions,
    ) -> Result<ApiResponse<PostListResponse>, CommandError> {
        let response: ApiResponse<PostsQueryResponse> = self.graphql(
            POSTS_QUERY,
            json!({
                "input": {
                    "organizationId": options.organization_id,
                    "filter": Value::Object(build_posts_filter(options)),
                },
                "first": options.limit,
                "after": options.cursor,
            }),
        )?;

        Ok(response.map(|payload| {
            let posts = payload
                .posts
                .edges
                .into_iter()
                .map(|edge| edge.node)
                .collect::<Vec<_>>();

            PostListResponse {
                posts,
                page_info: PageInfo {
                    has_more: payload.posts.page_info.has_next_page,
                    next_cursor: payload.posts.page_info.end_cursor,
                },
            }
        }))
    }

    pub fn get_post(&self, post_id: &str) -> Result<ApiResponse<Option<Post>>, CommandError> {
        let response: ApiResponse<PostQueryResponse> = self.graphql(
            POST_QUERY,
            json!({
                "input": {
                    "id": post_id,
                }
            }),
        )?;
        Ok(response.map(|payload| payload.post))
    }

    pub fn daily_posting_limits(
        &self,
        channel_ids: &[String],
        date: Option<&str>,
    ) -> Result<ApiResponse<Vec<DailyPostingLimitStatus>>, CommandError> {
        let response: ApiResponse<DailyPostingLimitsQueryResponse> = self.graphql(
            DAILY_POSTING_LIMITS_QUERY,
            json!({
                "input": {
                    "channelIds": channel_ids,
                    "date": date,
                }
            }),
        )?;

        Ok(response.map(|payload| payload.daily_posting_limits))
    }

    pub fn create_post(&self, input: Value) -> Result<ApiResponse<Post>, CommandError> {
        let response: ApiResponse<CreatePostMutationResponse> =
            self.graphql(CREATE_POST_MUTATION, json!({ "input": input }))?;
        let warnings = response.warnings;
        let payload = response.data.create_post;
        let typename = payload
            .get("__typename")
            .and_then(Value::as_str)
            .unwrap_or("Unknown")
            .to_owned();
        let message = payload
            .get("message")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);

        let post = match typename.as_str() {
            "PostActionSuccess" => {
                let post_value = payload.get("post").cloned().ok_or_else(|| {
                    CommandError::failure(
                        "BUFFER_API_ERROR",
                        "Buffer createPost response did not include a post",
                        "Inspect the response payload and retry",
                    )
                })?;
                serde_json::from_value(post_value).map_err(|error| {
                    CommandError::failure(
                        "BUFFER_API_ERROR",
                        format!("failed to decode created post: {error}"),
                        "Retry the command after inspecting the response payload",
                    )
                })
            }
            "UnauthorizedError" => Err(CommandError::blocked(
                "UNAUTHORIZED",
                payload_message(&payload, "Buffer rejected the API token"),
                "Refresh BUF_API_TOKEN and retry",
            )),
            "LimitReachedError" => Err(CommandError::failure(
                "LIMIT_REACHED",
                payload_message(&payload, "Buffer account limit reached"),
                "Reduce scheduled post volume or retry after plan limits reset",
            )),
            "InvalidInputError" => Err(CommandError::failure(
                "VALIDATION_ERROR",
                payload_message(&payload, "Buffer rejected the createPost input"),
                "Adjust the request fields and retry",
            )
            .with_details(payload)),
            "NotFoundError" => Err(CommandError::failure(
                "NOT_FOUND",
                payload_message(&payload, "Buffer resource was not found"),
                "Check the requested channel or post id and retry",
            )),
            "RestProxyError" => Err(CommandError::failure(
                "BUFFER_PROXY_ERROR",
                payload_message(&payload, "Buffer upstream proxy returned an error"),
                "Retry the request or reduce unsupported fields",
            )),
            "UnexpectedError" => Err(CommandError::failure(
                "BUFFER_API_ERROR",
                payload_message(&payload, "Buffer returned an unexpected error"),
                "Retry the request after inspecting the returned message",
            )),
            _ => match message {
                Some(message) => Err(CommandError::failure(
                    "BUFFER_MUTATION_ERROR",
                    message,
                    "Adjust the request fields or inspect the Buffer response and retry",
                )
                .with_details(payload)),
                None => Err(CommandError::failure(
                    "BUFFER_API_ERROR",
                    format!("unsupported createPost payload type `{typename}`"),
                    "Inspect the Buffer response and update the client mapping if the API changed",
                )
                .with_details(payload)),
            },
        }?;

        Ok(ApiResponse::new(post, warnings))
    }

    pub fn delete_post(&self, post_id: &str) -> Result<ApiResponse<String>, CommandError> {
        let response: ApiResponse<DeletePostMutationResponse> =
            self.graphql(DELETE_POST_MUTATION, json!({ "input": { "id": post_id } }))?;
        let warnings = response.warnings;
        let payload = response.data.delete_post;
        let typename = payload
            .get("__typename")
            .and_then(Value::as_str)
            .unwrap_or("Unknown")
            .to_owned();
        let message = payload
            .get("message")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);

        let deleted_id = match typename.as_str() {
            "DeletePostSuccess" => payload
                .get("id")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .ok_or_else(|| {
                    CommandError::failure(
                        "BUFFER_API_ERROR",
                        "Buffer deletePost response did not include an id",
                        "Inspect the response payload and retry",
                    )
                }),
            _ => match message {
                Some(message) => Err(map_delete_post_error(&typename, &message, payload)),
                None => Err(CommandError::failure(
                    "BUFFER_API_ERROR",
                    format!("unsupported deletePost payload type `{typename}`"),
                    "Inspect the Buffer response and update the client mapping if the API changed",
                )
                .with_details(payload)),
            },
        }?;

        Ok(ApiResponse::new(deleted_id, warnings))
    }

    fn graphql<T: DeserializeOwned>(
        &self,
        query: &str,
        variables: Value,
    ) -> Result<ApiResponse<T>, CommandError> {
        let payload = json!({
            "query": query,
            "variables": variables,
        });

        let response = self
            .http
            .post(&self.base_url)
            .bearer_auth(&self.token)
            .json(&payload)
            .send()
            .map_err(|error| {
                CommandError::failure(
                    "NETWORK_ERROR",
                    format!("failed to reach Buffer API: {error}"),
                    format!("Check network access and {}", self.base_url),
                )
            })?;

        let status = response.status();
        let body = response.text().map_err(|error| {
            CommandError::failure(
                "NETWORK_ERROR",
                format!("failed to read Buffer API response: {error}"),
                "Retry the request after confirming Buffer API availability",
            )
        })?;

        if !status.is_success() {
            return Err(map_http_error(status, &body));
        }

        let envelope: GraphqlEnvelope<T> = serde_json::from_str(&body).map_err(|error| {
            CommandError::failure(
                "BUFFER_API_ERROR",
                format!("failed to decode Buffer API response: {error}"),
                "Retry the request after inspecting the response body",
            )
            .with_details(json!({ "body": body }))
        })?;

        let warnings = match envelope.errors {
            Some(errors) if !errors.is_empty() => {
                if envelope.data.is_none() {
                    return Err(map_graphql_error(&errors));
                }
                graphql_warnings(&errors)
            }
            _ => Vec::new(),
        };

        let data = envelope.data.ok_or_else(|| {
            CommandError::failure(
                "BUFFER_API_ERROR",
                "Buffer API returned no data",
                "Retry the request after inspecting the response body",
            )
            .with_details(json!({ "body": body }))
        })?;

        Ok(ApiResponse::new(data, warnings))
    }
}

#[derive(Debug, Deserialize)]
struct GraphqlEnvelope<T> {
    data: Option<T>,
    errors: Option<Vec<GraphqlError>>,
}

#[derive(Debug, Deserialize)]
struct GraphqlError {
    message: String,
    extensions: Option<GraphqlErrorExtensions>,
}

#[derive(Debug, Deserialize)]
struct GraphqlErrorExtensions {
    code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AccountQueryResponse {
    account: Account,
}

#[derive(Debug, Deserialize)]
struct Account {
    organizations: Vec<Organization>,
}

#[derive(Debug, Deserialize)]
struct ChannelsQueryResponse {
    channels: Vec<Channel>,
}

#[derive(Debug, Deserialize)]
struct ChannelQueryResponse {
    channel: Option<Channel>,
}

#[derive(Debug, Deserialize)]
struct PostsQueryResponse {
    posts: PostsConnection,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DailyPostingLimitsQueryResponse {
    daily_posting_limits: Vec<DailyPostingLimitStatus>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PostsConnection {
    edges: Vec<PostEdge>,
    page_info: GraphqlPageInfo,
}

#[derive(Debug, Deserialize)]
struct PostEdge {
    #[allow(dead_code)]
    cursor: String,
    node: Post,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlPageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PostQueryResponse {
    post: Option<Post>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatePostMutationResponse {
    create_post: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeletePostMutationResponse {
    delete_post: Value,
}

fn map_http_error(status: StatusCode, body: &str) -> CommandError {
    match status {
        StatusCode::UNAUTHORIZED => CommandError::blocked(
            "UNAUTHORIZED",
            "Buffer rejected the API token",
            "Refresh BUF_API_TOKEN and retry",
        )
        .with_details(json!({ "status": status.as_u16(), "body": body })),
        StatusCode::FORBIDDEN => CommandError::blocked(
            "FORBIDDEN",
            "Buffer rejected the request for this resource",
            "Verify the API key permissions and the requested resource ownership",
        )
        .with_details(json!({ "status": status.as_u16(), "body": body })),
        StatusCode::NOT_FOUND => CommandError::failure(
            "NOT_FOUND",
            "Buffer resource was not found",
            "Check the requested id and retry",
        )
        .with_details(json!({ "status": status.as_u16(), "body": body })),
        StatusCode::TOO_MANY_REQUESTS => CommandError::blocked(
            "RATE_LIMITED",
            "Buffer API rate limit reached",
            "Retry after the current rate-limit window resets or the documented retryAfter interval",
        )
        .with_details(json!({ "status": status.as_u16(), "body": body })),
        _ => CommandError::failure(
            "BUFFER_API_ERROR",
            format!("Buffer API returned HTTP {}", status.as_u16()),
            "Retry the request after inspecting the response body",
        )
        .with_details(json!({ "status": status.as_u16(), "body": body })),
    }
}

fn map_graphql_error(errors: &[GraphqlError]) -> CommandError {
    let first = &errors[0];
    let code = first
        .extensions
        .as_ref()
        .and_then(|extensions| extensions.code.as_deref())
        .unwrap_or("");
    let details = json!({
        "errors": errors.iter().map(|error| {
            json!({
                "message": error.message,
                "code": error.extensions.as_ref().and_then(|extensions| extensions.code.clone()),
            })
        }).collect::<Vec<_>>()
    });

    match code {
        "UNAUTHORIZED" => CommandError::blocked(
            "UNAUTHORIZED",
            first.message.clone(),
            "Refresh BUF_API_TOKEN and retry",
        )
        .with_details(details),
        "FORBIDDEN" => CommandError::blocked(
            "FORBIDDEN",
            first.message.clone(),
            "Verify the API key permissions and the requested resource ownership",
        )
        .with_details(details),
        "NOT_FOUND" => CommandError::failure(
            "NOT_FOUND",
            first.message.clone(),
            "Check the requested id and retry",
        )
        .with_details(details),
        "RATE_LIMIT_EXCEEDED" => CommandError::blocked(
            "RATE_LIMITED",
            first.message.clone(),
            "Retry after the current Buffer API rate-limit window resets or the documented retryAfter interval",
        )
        .with_details(details),
        "UNEXPECTED" => CommandError::failure(
            "BUFFER_API_ERROR",
            first.message.clone(),
            "Retry after a short delay and inspect the full GraphQL response if the problem persists",
        )
        .with_details(details),
        _ if first.message.to_ascii_lowercase().contains("unauthorized") => CommandError::blocked(
            "UNAUTHORIZED",
            first.message.clone(),
            "Refresh BUF_API_TOKEN and retry",
        )
        .with_details(details),
        _ if first.message.to_ascii_lowercase().contains("rate") => CommandError::blocked(
            "RATE_LIMITED",
            first.message.clone(),
            "Retry after the current Buffer API rate-limit window resets or the documented retryAfter interval",
        )
        .with_details(details),
        _ => CommandError::failure(
            "BUFFER_API_ERROR",
            first.message.clone(),
            "Inspect the returned GraphQL errors and retry with a smaller or simpler request",
        )
        .with_details(details),
    }
}

fn graphql_warnings(errors: &[GraphqlError]) -> Vec<BufferWarning> {
    errors
        .iter()
        .map(|error| BufferWarning {
            message: error.message.clone(),
            code: error
                .extensions
                .as_ref()
                .and_then(|extensions| extensions.code.clone()),
        })
        .collect()
}

fn map_delete_post_error(typename: &str, message: &str, payload: Value) -> CommandError {
    match typename {
        "UnauthorizedError" => CommandError::blocked(
            "UNAUTHORIZED",
            message.to_owned(),
            "Refresh BUF_API_TOKEN and retry",
        )
        .with_details(payload),
        "NotFoundError" => CommandError::failure(
            "NOT_FOUND",
            message.to_owned(),
            "Check the requested post id and retry",
        )
        .with_details(payload),
        _ => CommandError::failure(
            "BUFFER_MUTATION_ERROR",
            message.to_owned(),
            "Inspect the Buffer response and retry",
        )
        .with_details(payload),
    }
}

fn payload_message(payload: &Value, fallback: &str) -> String {
    payload
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or(fallback)
        .to_owned()
}

fn build_posts_filter(options: &ListPostsOptions) -> serde_json::Map<String, Value> {
    let mut filter = serde_json::Map::new();
    if !options.channel_ids.is_empty() {
        filter.insert("channelIds".to_owned(), json!(options.channel_ids));
    }
    if let Some(status) = options.status {
        filter.insert("status".to_owned(), json!([status.as_str()]));
    }
    apply_due_at_filter(&mut filter, options.from.as_deref(), options.to.as_deref());
    filter
}

fn apply_due_at_filter(
    filter: &mut serde_json::Map<String, Value>,
    from: Option<&str>,
    to: Option<&str>,
) {
    let mut comparator = serde_json::Map::new();
    if let Some(value) = from {
        comparator.insert("start".to_owned(), json!(value));
    }
    if let Some(value) = to {
        comparator.insert("end".to_owned(), json!(value));
    }
    if !comparator.is_empty() {
        filter.insert("dueAt".to_owned(), Value::Object(comparator));
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use crate::cli::PostStatus;

    use super::{ListPostsOptions, build_posts_filter};

    #[test]
    fn posts_filter_uses_nested_due_at_comparator() {
        let filter = build_posts_filter(&ListPostsOptions {
            organization_id: "org_123".to_owned(),
            channel_ids: vec!["ch_123".to_owned()],
            status: Some(PostStatus::Scheduled),
            from: Some("2026-03-09T00:00:00Z".to_owned()),
            to: Some("2026-03-16T00:00:00Z".to_owned()),
            limit: 20,
            cursor: None,
        });

        assert_eq!(filter.get("channelIds"), Some(&json!(["ch_123"])));
        assert_eq!(filter.get("status"), Some(&json!(["scheduled"])));
        assert_eq!(
            filter.get("dueAt"),
            Some(&json!({
                "start": "2026-03-09T00:00:00Z",
                "end": "2026-03-16T00:00:00Z",
            }))
        );
        assert_eq!(filter.get("dueAtStart"), None);
        assert_eq!(filter.get("dueAtStop"), None);
        assert_eq!(filter.get("publishedAtStart"), None);
        assert_eq!(filter.get("publishedAtStop"), None);
    }

    #[test]
    fn posts_filter_omits_due_at_when_no_bounds_are_provided() {
        let filter = build_posts_filter(&ListPostsOptions {
            organization_id: "org_123".to_owned(),
            channel_ids: vec![],
            status: Some(PostStatus::Sent),
            from: None,
            to: None,
            limit: 20,
            cursor: None,
        });

        assert_eq!(
            filter.get("status"),
            Some(&Value::Array(vec![json!("sent")]))
        );
        assert_eq!(filter.get("dueAt"), None);
    }
}
