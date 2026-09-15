//! Xendit payment gateway client (Bagian V — Pesan Ustadz).
//!
//! Dua mode:
//!  * LIVE/TEST : `XENDIT_SECRET_KEY` terisi → panggil https://api.xendit.co (Basic auth).
//!  * MOCK      : secret kosong → dev tanpa akun Xendit; invoice_url `mock://invoice/...`,
//!                refund sukses instan, dan e2e via `POST /payments/dev/simulate`
//!                (gate `MQ_DEV_EXPOSE_TOKENS=true`).
//!
//! Fakta API diverifikasi di V0 spike (plan Bagian V): invoice v2, webhook x-callback-token,
//! refund. Kalau format berubah, hanya file ini yang disentuh.

use serde::Deserialize;

#[derive(Clone)]
pub struct PaymentGateway {
    http: reqwest::Client,
    secret: Option<String>,
    base: String,
    pub callback_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Invoice {
    pub id: String,
    pub external_id: String,
    pub status: String,       // PENDING | PAID | EXPIRED
    pub invoice_url: String,
    pub amount: i64,
}

#[derive(Debug, Clone)]
pub struct RefundResult {
    pub id: String,
    pub amount: i64,
}

#[derive(Debug)]
pub struct CreateInvoice<'a> {
    pub external_id: &'a str,
    pub amount: i64,
    pub description: &'a str,
    pub duration_sec: i64,
    pub success_url: Option<&'a str>,
    pub failure_url: Option<&'a str>,
    pub payer_email: Option<&'a str>,
}

fn gw_err(context: &str, e: Option<&reqwest::Error>) -> String {
    match e {
        Some(e) => format!("{context}: {e}"),
        None => context.to_string(),
    }
}

impl PaymentGateway {
    pub fn from_env() -> Self {
        let secret = std::env::var("XENDIT_SECRET_KEY").ok().filter(|s| !s.trim().is_empty());
        let base = std::env::var("XENDIT_BASE_URL")
            .unwrap_or_else(|_| "https://api.xendit.co".to_string());
        let callback_token = std::env::var("XENDIT_CALLBACK_TOKEN").ok().filter(|s| !s.trim().is_empty());
        if secret.is_none() {
            tracing::warn!("XENDIT_SECRET_KEY kosong -> PAYMENT GATEWAY MODE MOCK (dev only)");
        }
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("reqwest client"),
            secret,
            base,
            callback_token,
        }
    }

    pub fn is_mock(&self) -> bool {
        self.secret.is_none()
    }

    fn auth(&self) -> (&str, Option<&str>) {
        // HTTP Basic: secret key sebagai username, password kosong (konvensi Xendit)
        (self.secret.as_deref().unwrap_or(""), Some(""))
    }

    pub async fn create_invoice(&self, req: CreateInvoice<'_>) -> Result<Invoice, String> {
        if self.is_mock() {
            return Ok(Invoice {
                id: format!("mockinv-{}", req.external_id),
                external_id: req.external_id.to_string(),
                status: "PENDING".into(),
                invoice_url: format!("mock://invoice/{}", req.external_id),
                amount: req.amount,
            });
        }
        #[derive(serde::Serialize)]
        struct Body<'b> {
            external_id: &'b str,
            amount: i64,
            description: &'b str,
            invoice_duration: i64,
            #[serde(skip_serializing_if = "Option::is_none")]
            success_redirect_url: Option<&'b str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            failure_redirect_url: Option<&'b str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            payer_email: Option<&'b str>,
            currency: &'static str,
        }
        let body = Body {
            external_id: req.external_id,
            amount: req.amount,
            description: req.description,
            invoice_duration: req.duration_sec,
            success_redirect_url: req.success_url,
            failure_redirect_url: req.failure_url,
            payer_email: req.payer_email,
            currency: "IDR",
        };
        let resp = self
            .http
            .post(format!("{}/v2/invoices", self.base))
            .basic_auth(self.auth().0, self.auth().1)
            .json(&body)
            .send()
            .await
            .map_err(|e| gw_err("xendit create_invoice", Some(&e)))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            tracing::error!("xendit create_invoice HTTP {status}: {text:.300}");
            return Err(format!("xendit create_invoice HTTP {status}"));
        }
        #[derive(Deserialize)]
        struct Out {
            id: String,
            external_id: String,
            status: String,
            invoice_url: String,
            amount: i64,
        }
        let out: Out = serde_json::from_str(&text).map_err(|e| format!("xendit response parse: {e}"))?;
        Ok(Invoice {
            id: out.id,
            external_id: out.external_id,
            status: out.status,
            invoice_url: out.invoice_url,
            amount: out.amount,
        })
    }

    pub async fn get_invoice(&self, id: &str) -> Result<Invoice, String> {
        if self.is_mock() {
            return Ok(Invoice {
                id: id.to_string(),
                external_id: id.to_string(),
                status: "PENDING".into(),
                invoice_url: String::new(),
                amount: 0,
            });
        }
        let resp = self
            .http
            .get(format!("{}/v2/invoices/{id}", self.base))
            .basic_auth(self.auth().0, self.auth().1)
            .send()
            .await
            .map_err(|e| gw_err("xendit get_invoice", Some(&e)))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("xendit get_invoice HTTP {status}"));
        }
        #[derive(Deserialize)]
        struct Out {
            id: String,
            external_id: String,
            status: String,
            invoice_url: String,
            amount: i64,
        }
        let out: Out = serde_json::from_str(&text).map_err(|e| format!("xendit response parse: {e}"))?;
        Ok(Invoice {
            id: out.id,
            external_id: out.external_id,
            status: out.status,
            invoice_url: out.invoice_url,
            amount: out.amount,
        })
    }

    pub async fn create_refund(&self, invoice_id: &str, amount: i64, reason: &str) -> Result<RefundResult, String> {
        if self.is_mock() {
            return Ok(RefundResult { id: format!("mockrf-{invoice_id}"), amount });
        }
        #[derive(serde::Serialize)]
        struct Body<'b> {
            invoice_id: &'b str, // field klasik; V0 verifikasi payment_request_id jika wajib
            amount: i64,
            reason: &'b str,
        }
        let resp = self
            .http
            .post(format!("{}/refunds", self.base))
            .basic_auth(self.auth().0, self.auth().1)
            .json(&Body { invoice_id, amount, reason })
            .send()
            .await
            .map_err(|e| gw_err("xendit create_refund", Some(&e)))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            tracing::error!("xendit create_refund HTTP {status}: {text:.300}");
            return Err(format!("xendit create_refund HTTP {status} (fitur refund mungkin belum aktif utk akun)"));
        }
        #[derive(Deserialize)]
        struct Out {
            id: String,
            amount: i64,
        }
        let out: Out = serde_json::from_str(&text).map_err(|e| format!("xendit refund parse: {e}"))?;
        Ok(RefundResult { id: out.id, amount: out.amount })
    }
}

/// Constant-time-ish comparison token webhook (anti timing).
pub fn token_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_eq_benar() {
        assert!(token_eq("abc123", "abc123"));
        assert!(!token_eq("abc123", "abc124"));
        assert!(!token_eq("abc", "abcd"));
    }

    #[test]
    fn mock_invoice_url() {
        let gw = PaymentGateway {
            http: reqwest::Client::new(),
            secret: None,
            base: String::new(),
            callback_token: None,
        };
        assert!(gw.is_mock());
    }
}
