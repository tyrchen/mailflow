/// Mock AWS services for integration testing
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Mock S3 client for testing
#[derive(Clone)]
pub struct MockS3 {
    pub objects: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl MockS3 {
    pub fn new() -> Self {
        Self {
            objects: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn put_object(&self, key: String, data: Vec<u8>) {
        self.objects.lock().unwrap().insert(key, data);
    }

    pub fn get_object(&self, key: &str) -> Option<Vec<u8>> {
        self.objects.lock().unwrap().get(key).cloned()
    }

    pub fn delete_object(&self, key: &str) {
        self.objects.lock().unwrap().remove(key);
    }

    pub fn list_objects(&self, prefix: &str) -> Vec<String> {
        self.objects
            .lock()
            .unwrap()
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect()
    }
}

impl Default for MockS3 {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock SQS client for testing
#[derive(Clone)]
pub struct MockSQS {
    pub queues: Arc<Mutex<HashMap<String, Vec<String>>>>,
}

impl MockSQS {
    pub fn new() -> Self {
        Self {
            queues: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn send_message(&self, queue_url: &str, message: String) {
        self.queues
            .lock()
            .unwrap()
            .entry(queue_url.to_string())
            .or_default()
            .push(message);
    }

    pub fn receive_message(&self, queue_url: &str) -> Option<String> {
        self.queues
            .lock()
            .unwrap()
            .get_mut(queue_url)
            .and_then(|q| {
                if q.is_empty() {
                    None
                } else {
                    Some(q.remove(0))
                }
            })
    }

    pub fn get_queue_length(&self, queue_url: &str) -> usize {
        self.queues
            .lock()
            .unwrap()
            .get(queue_url)
            .map(|q| q.len())
            .unwrap_or(0)
    }

    pub fn purge_queue(&self, queue_url: &str) {
        self.queues
            .lock()
            .unwrap()
            .entry(queue_url.to_string())
            .or_default()
            .clear();
    }
}

impl Default for MockSQS {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock DynamoDB client for testing
#[derive(Clone)]
pub struct MockDynamoDB {
    pub items: Arc<Mutex<HashMap<String, HashMap<String, String>>>>,
}

impl MockDynamoDB {
    pub fn new() -> Self {
        Self {
            items: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn put_item(&self, table: &str, key: String, value: HashMap<String, String>) {
        let table_key = format!("{}#{}", table, key);
        self.items.lock().unwrap().insert(table_key, value);
    }

    pub fn get_item(&self, table: &str, key: &str) -> Option<HashMap<String, String>> {
        let table_key = format!("{}#{}", table, key);
        self.items.lock().unwrap().get(&table_key).cloned()
    }

    pub fn delete_item(&self, table: &str, key: &str) {
        let table_key = format!("{}#{}", table, key);
        self.items.lock().unwrap().remove(&table_key);
    }

    pub fn item_exists(&self, table: &str, key: &str) -> bool {
        let table_key = format!("{}#{}", table, key);
        self.items.lock().unwrap().contains_key(&table_key)
    }
}

impl Default for MockDynamoDB {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock SES client for testing
#[derive(Clone)]
pub struct MockSES {
    pub sent_emails: Arc<Mutex<Vec<SentEmail>>>,
    pub verified_addresses: Arc<Mutex<Vec<String>>>,
}

#[derive(Debug, Clone)]
pub struct SentEmail {
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,
    pub body: String,
    pub message_id: String,
}

impl MockSES {
    pub fn new() -> Self {
        Self {
            sent_emails: Arc::new(Mutex::new(Vec::new())),
            verified_addresses: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn add_verified_address(&self, address: String) {
        self.verified_addresses.lock().unwrap().push(address);
    }

    pub fn is_verified(&self, address: &str) -> bool {
        self.verified_addresses
            .lock()
            .unwrap()
            .iter()
            .any(|a| a == address)
    }

    pub fn send_email(&self, email: SentEmail) -> String {
        let message_id = email.message_id.clone();
        self.sent_emails.lock().unwrap().push(email);
        message_id
    }

    pub fn get_sent_count(&self) -> usize {
        self.sent_emails.lock().unwrap().len()
    }

    pub fn clear_sent_emails(&self) {
        self.sent_emails.lock().unwrap().clear();
    }
}

impl Default for MockSES {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_s3() {
        let s3 = MockS3::new();
        s3.put_object("test/file.txt".to_string(), b"content".to_vec());
        assert_eq!(s3.get_object("test/file.txt"), Some(b"content".to_vec()));
        assert_eq!(s3.list_objects("test/").len(), 1);
    }

    #[test]
    fn test_mock_sqs() {
        let sqs = MockSQS::new();
        sqs.send_message("queue1", "message1".to_string());
        assert_eq!(sqs.get_queue_length("queue1"), 1);
        assert_eq!(sqs.receive_message("queue1"), Some("message1".to_string()));
        assert_eq!(sqs.get_queue_length("queue1"), 0);
    }

    #[test]
    fn test_mock_dynamodb() {
        let db = MockDynamoDB::new();
        let mut item = HashMap::new();
        item.insert("field".to_string(), "value".to_string());
        db.put_item("table1", "key1".to_string(), item.clone());
        assert!(db.item_exists("table1", "key1"));
        assert_eq!(db.get_item("table1", "key1"), Some(item));
    }

    #[test]
    fn test_mock_ses() {
        let ses = MockSES::new();
        ses.add_verified_address("test@example.com".to_string());
        assert!(ses.is_verified("test@example.com"));
        assert!(!ses.is_verified("other@example.com"));
    }
}
