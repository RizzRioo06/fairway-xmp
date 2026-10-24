//! Remote mail images. The renderer keeps its data-only image CSP.
use crate::config::Result;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use reqwest::{header, redirect, Client, Url};
use std::time::Duration;

const IMAGE_LIMIT: usize = 2 * 1024 * 1024;

fn checked_url(value: &str) -> Result<Url> {
    let invalid = "Only HTTP/HTTPS image URLs without credentials can be loaded.";
    if value.len() > 8192 || value.chars().any(char::is_control) {
        return Err(invalid.into());
    }
    let mut url = Url::parse(value).map_err(|_| invalid.to_string())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(invalid.into());
    }
    url.set_fragment(None);
    Ok(url)
}

fn raster_mime(mime: &str) -> bool {
    matches!(
        mime,
        "image/png"
            | "image/jpeg"
            | "image/gif"
            | "image/webp"
            | "image/avif"
            | "image/bmp"
            | "image/x-icon"
            | "image/vnd.microsoft.icon"
    )
}

pub async fn load(value: &str) -> Result<String> {
    let mut url = checked_url(value)?;
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    // Separate cookie-free client: no browser credentials, Referer, automatic
    // redirects, retries, proxies or response decompression. Local development
    // addresses are intentionally supported for local outgoing-mail debugging.
    let client = Client::builder()
        .redirect(redirect::Policy::none())
        .retry(reqwest::retry::never())
        .referer(false)
        .no_proxy()
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| "Could not initialize the image loader.")?;
    for hop in 0..=3 {
        let mut response = client
            .get(url.clone())
            .header(
                header::ACCEPT,
                "image/png,image/jpeg,image/gif,image/webp,image/avif,image/bmp,image/x-icon",
            )
            .send()
            .await
            .map_err(|_| "Could not download this image. Check its address and server.")?;
        if response.status().is_redirection() {
            if hop == 3 {
                return Err("This image redirected too many times.".into());
            }
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or("Invalid image redirect.")?;
            let next = checked_url(
                url.join(location)
                    .map_err(|_| "Invalid image redirect.")?
                    .as_str(),
            )?;
            if url.scheme() == "https" && next.scheme() != "https" {
                return Err("An HTTPS image cannot redirect to an insecure address.".into());
            }
            url = next;
            continue;
        }
        if !response.status().is_success() {
            return Err(format!(
                "Image server returned HTTP {}.",
                response.status().as_u16()
            ));
        }
        let mime = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if !raster_mime(&mime) {
            return Err(
                "This response is not a supported raster image. SVG and HTML stay blocked.".into(),
            );
        }
        if response
            .content_length()
            .is_some_and(|length| length > IMAGE_LIMIT as u64)
        {
            return Err("This image exceeds the 2 MiB preview limit.".into());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "The image download was incomplete.")?
        {
            if bytes.len() + chunk.len() > IMAGE_LIMIT {
                return Err("This image exceeds the 2 MiB preview limit.".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.is_empty() {
            return Err("The image server returned an empty image.".into());
        }
        return Ok(format!("data:{mime};base64,{}", STANDARD.encode(bytes)));
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };

    #[test]
    fn image_urls_reject_credentials_other_schemes_and_control_characters() {
        for value in [
            "file:///tmp/logo.png",
            "data:image/png;base64,AA==",
            "http://u:p@localhost/img",
            "https://example.test/\nimg",
            "//example.test/img",
        ] {
            assert!(checked_url(value).is_err(), "{value}");
        }
        assert_eq!(
            checked_url("http://localhost:8800/logo.png#x")
                .unwrap()
                .as_str(),
            "http://localhost:8800/logo.png"
        );
        assert!(!raster_mime("image/svg+xml"));
        assert!(!raster_mime("text/html"));
    }

    #[test]
    fn explicit_image_downloads_are_bounded_cookie_free_and_validate_redirects() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let responses = [
                "HTTP/1.1 302 Found\r\nLocation: /image\r\nSet-Cookie: token=secret\r\nContent-Length: 0\r\n",
                "HTTP/1.1 200 OK\r\nContent-Type: image/gif\r\nContent-Length: 6\r\n",
                "HTTP/1.1 200 OK\r\nContent-Type: image/svg+xml\r\nContent-Length: 0\r\n",
                "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 2097153\r\n",
                "HTTP/1.1 302 Found\r\nLocation: http://user:pass@localhost/image\r\nContent-Length: 0\r\n",
                "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 100\r\n",
                "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\n",
            ];
            for (index, response) in responses.iter().enumerate() {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    socket.read_exact(&mut byte).unwrap();
                    request.extend(byte);
                }
                let request = String::from_utf8(request).unwrap().to_ascii_lowercase();
                for header in ["\r\ncookie:", "\r\nauthorization:", "\r\nreferer:"] {
                    assert!(!request.contains(header), "{request}");
                }
                write!(
                    socket,
                    "{response}Connection: close\r\n\r\n{}",
                    if index == 1 { "GIF89a" } else { "" }
                )
                .unwrap();
                if index == 6 {
                    // No advertised length: the streaming limit must still win.
                    let _ = socket.write_all(&vec![0; IMAGE_LIMIT + 1]);
                }
            }
        });
        tauri::async_runtime::block_on(async {
            assert_eq!(
                load(&format!("{base}/redirect")).await.unwrap(),
                "data:image/gif;base64,R0lGODlh"
            );
            assert!(load(&format!("{base}/svg"))
                .await
                .unwrap_err()
                .contains("raster"));
            assert!(load(&format!("{base}/large"))
                .await
                .unwrap_err()
                .contains("2 MiB"));
            assert!(load(&format!("{base}/credentials")).await.is_err());
            assert!(load(&format!("{base}/truncated")).await.is_err());
            assert!(load(&format!("{base}/streamed-large"))
                .await
                .unwrap_err()
                .contains("2 MiB"));
        });
        server.join().unwrap();
    }
}
