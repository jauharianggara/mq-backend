//! Xendit invoice client (Panggil Ustadz v2 — Bagian W1).
//!
//! Dua mode:
//!  * LIVE/TEST : `XENDIT_SECRET_KEY` terisi -> https://api.xendit.co (Basic auth).
//!  * MOCK      : secret kosong (dev sebelum akun Xendit) -> invoice_url `mock://invoice/{external}`,
//!                status selalu PENDING; pembayaran disimulasikan lewat
//!                `POST /payments/xendit/simulate` (hanya saat MQ_DEV_EXPOSE_TOKENS=true).
//!
//! Webhook: header `x-callback-token` dibandingkan constant-time dgn `XENDIT_CALLBACK_TOKEN`.
//! Refund API Xendit TIDAK dipakai di v2 — semua pengembalian dana = kredit deposit santri.

#[derive(Debug, serde::Serialize)]
pub struct CreateInvoice<'a> {
    pub external_id: &'a str,
    pub amount: i64,
    pub description: &'a str,
    pub duration_sec: i64,
}

#[derive(Clone)]
pub struct PaymentGateway {
    pub secret: Option<String>,
    pub callback_token: Option<String>,
    pub base: String,
    pub http: reqwest::Client,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Invoice {
    pub id: String,
    pub external_id: String,
    pub status: String, // PENDING | PAID | EXPIRED
    pub invoice_url: String,
    pub amount: i64,
}

pub fn token_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

impl PaymentGateway {
    pub fn from_env() -> Self {
        let secret = std::env::var("XENDIT_SECRET_KEY").ok().filter(|s| !s.trim().is_empty());
        let base = std::env::var("XENDIT_BASE_URL").unwrap_or_else(|_| "https://api.xendit.co".into());
        let callback_token =
            std::env::var("XENDIT_CALLBACK_TOKEN").ok().filter(|s| !s.trim().is_empty());
        if secret.is_none() {
            tracing::warn!("XENDIT_SECRET_KEY kosong -> Xendit MODE MOCK (dev only)");
        }
        Self {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
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

        pub async fn create_invoice(&self, req: CreateInvoice<'_>) -> Result<Invoice, String> {
        if self.is_mock() {
            return Ok(Invoice {
                id: format!("mockinv-{}", req.external_id),
                external_id: req.external_id.into(),
                status: "PENDING".into(),
                invoice_url: format!("mock://invoice/{}", req.external_id),
                amount: req.amount,
            });
        }
        let resp = self
            .http
            .post(format!("{}/v2/invoices", self.base))
            .basic_auth(self.secret.as_deref().unwrap_or(""), Some(""))
            .json(&req)
            .send()
            .await
            .map_err(|e| format!("xendit create_invoice: {e}"))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            tracing::error!("xendit create_invoice HTTP {status}: {}", &text[..text.len().min(300)]);
            return Err(format!("xendit create_invoice HTTP {status}"));
        }
        #[derive(serde::Deserialize)]
        struct Out {
            id: String,
            external_id: String,
            status: String,
            invoice_url: String,
            amount: i64,
        }
        let o: Out = serde_json::from_str(&text).map_err(|e| format!("xendit parse: {e}"))?;
        Ok(Invoice { id: o.id, external_id: o.external_id, status: o.status, invoice_url: o.invoice_url, amount: o.amount })
    }

pub async fn get_invoice(&self, invoice_id: &str) -> Result<Invoice, String> {
        if self.is_mock() {
            return Ok(Invoice {
                id: invoice_id.into(),
                external_id: String::new(),
                status: "PENDING".into(),
                invoice_url: String::new(),
                amount: 0,
            });
        }
        let resp = self
            .http
            .get(format!("{}/v2/invoices/{invoice_id}", self.base))
            .basic_auth(self.secret.as_deref().unwrap_or(""), Some(""))
            .send()
            .await
            .map_err(|e| format!("xendit get_invoice: {e}"))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("xendit get_invoice HTTP {status}"));
        }
        #[derive(serde::Deserialize)]
        struct Out {
            id: String,
            external_id: String,
            status: String,
            invoice_url: String,
            amount: i64,
        }
        let o: Out = serde_json::from_str(&text).map_err(|e| format!("xendit parse: {e}"))?;
        Ok(Invoice { id: o.id, external_id: o.external_id, status: o.status, invoice_url: o.invoice_url, amount: o.amount })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_eq_benar() {
        assert!(token_eq("abc", "abc"));
        assert!(!token_eq("abc", "abd"));
        assert!(!token_eq("ab", "abc"));
    }

    #[test]
    fn mock_invoice() {
        let gw = PaymentGateway {
            http: reqwest::Client::new(),
            secret: None,
            callback_token: None,
            base: String::new(),
        };
        assert!(gw.is_mock());
    }
}
