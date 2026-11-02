use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn gateway_posts_the_original_job_status_and_reports_http_failure() {
    for status in ["200 OK", "503 Service Unavailable"] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = ConnectionGatewayClient::new(
            "test-fleet-key".into(),
            format!("http://{}", listener.local_addr().unwrap()),
        );
        let result = serde_json::json!({"jobId":"job-one","jobType":"docx_upload","status":"Completed","data":{"error":false,"data":{"converted":true}}});
        let expected = result.clone();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let header_end = loop {
                let count = socket.read_buf(&mut bytes).await.unwrap();
                assert_ne!(count, 0);
                if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                    break end + 4;
                }
            };
            let headers = std::str::from_utf8(&bytes[..header_end]).unwrap();
            assert!(headers.starts_with("POST /message/send/document/document-one HTTP/1.1"));
            assert!(
                headers
                    .to_ascii_lowercase()
                    .contains("x-internal-auth-key: test-fleet-key")
            );
            let length: usize = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length: ")
                        .map(str::parse)
                })
                .unwrap()
                .unwrap();
            while bytes.len() < header_end + length {
                assert_ne!(socket.read_buf(&mut bytes).await.unwrap(), 0);
            }
            let body: serde_json::Value =
                serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap();
            assert_eq!(
                body,
                serde_json::json!({"message_type":"job_update","message":expected})
            );
            socket.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: 15\r\nConnection: close\r\n\r\n{{\"receipts\":[]}}").as_bytes()).await.unwrap();
        });
        let delivered = client.send_job_update("document-one", result).await;
        server.await.unwrap();
        assert_eq!(delivered.is_ok(), status == "200 OK");
    }
}
