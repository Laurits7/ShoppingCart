//! Talking to Mealie (SPEC §6.10), as documented by its OpenAPI on 2026-10-18 (v3.29):
//! shopping lists and items live under `/api/households/shopping/…`, authenticated with an
//! API token as a bearer token.

use serde::Deserialize;
use serde_json::Value;
use shoppingcart_domain::mealie::MealieItem;

use crate::net::body_limited;

const MAX_ANSWER: usize = 5 * 1024 * 1024;

pub struct Mealie<'a> {
    pub http: &'a reqwest::Client,
    pub base: &'a str,
    pub token: &'a str,
}

#[derive(Debug, Clone, serde::Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[ts(export)]
pub struct MealieList {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize)]
struct Page<T> {
    items: Vec<T>,
    #[serde(default)]
    total_pages: Option<i64>,
}

impl Mealie<'_> {
    fn explain(e: reqwest::Error) -> String {
        if e.is_timeout() {
            "Mealie didn't answer in time".into()
        } else if e.is_connect() {
            "Mealie couldn't be reached at that address".into()
        } else {
            format!("couldn't talk to Mealie ({e})")
        }
    }

    async fn check(res: reqwest::Response) -> Result<reqwest::Response, String> {
        match res.status().as_u16() {
            200..=299 => Ok(res),
            401 | 403 => Err("Mealie refused the API token".into()),
            404 => {
                Err("Mealie didn't know this (is the address right, and Mealie 2 or newer?)".into())
            }
            s => Err(format!("Mealie answered {s}")),
        }
    }

    async fn get<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, String> {
        let res = self
            .http
            .get(format!("{}{path}", self.base))
            .bearer_auth(self.token)
            .query(query)
            .send()
            .await
            .map_err(Self::explain)?;
        let body = body_limited(Self::check(res).await?, MAX_ANSWER).await?;
        serde_json::from_slice(&body).map_err(|_| "Mealie gave an answer we can't read".to_string())
    }

    /// The shopping lists the token can see.
    pub async fn lists(&self) -> Result<Vec<MealieList>, String> {
        #[derive(Deserialize)]
        struct L {
            id: String,
            name: Option<String>,
        }
        let p: Page<L> = self
            .get(
                "/api/households/shopping/lists",
                &[("perPage", "200".into())],
            )
            .await?;
        Ok(p.items
            .into_iter()
            .map(|l| MealieList {
                name: l.name.unwrap_or_else(|| "(no name)".into()),
                id: l.id,
            })
            .collect())
    }

    /// The unchecked items of one list, as Mealie sent them.
    pub async fn unchecked(&self, list_id: &str) -> Result<Vec<Value>, String> {
        let filter = format!("shoppingListId=\"{list_id}\" AND checked=false");
        let mut out = vec![];
        for page in 1..=20 {
            let p: Page<Value> = self
                .get(
                    "/api/households/shopping/items",
                    &[
                        ("queryFilter", filter.clone()),
                        ("perPage", "200".into()),
                        ("page", page.to_string()),
                    ],
                )
                .await?;
            let n = p.items.len();
            // Filter again here, whatever the server did with the filter.
            out.extend(p.items.into_iter().filter(|i| {
                i.get("shoppingListId").and_then(Value::as_str) == Some(list_id)
                    && !i.get("checked").and_then(Value::as_bool).unwrap_or(false)
            }));
            if n == 0 || p.total_pages.is_none_or(|t| page >= t) {
                break;
            }
        }
        Ok(out)
    }

    /// Tick items in Mealie: each sent back exactly as received, with `checked: true`, so no
    /// other field is reset.
    pub async fn check_items(&self, items: &[Value]) -> Result<(), String> {
        if items.is_empty() {
            return Ok(());
        }
        let body: Vec<Value> = items
            .iter()
            .map(|i| {
                let mut i = i.clone();
                i["checked"] = Value::Bool(true);
                i
            })
            .collect();
        let res = self
            .http
            .put(format!("{}/api/households/shopping/items", self.base))
            .bearer_auth(self.token)
            .json(&body)
            .send()
            .await
            .map_err(Self::explain)?;
        Self::check(res).await.map(|_| ())
    }

    pub async fn delete_items(&self, ids: &[String]) -> Result<(), String> {
        if ids.is_empty() {
            return Ok(());
        }
        let query: Vec<(&str, &String)> = ids.iter().map(|i| ("ids", i)).collect();
        let res = self
            .http
            .delete(format!("{}/api/households/shopping/items", self.base))
            .bearer_auth(self.token)
            .query(&query)
            .send()
            .await
            .map_err(Self::explain)?;
        Self::check(res).await.map(|_| ())
    }
}

/// The parts of a Mealie item the mapping uses.
pub fn parts(v: &Value) -> MealieItem {
    let s = |v: Option<&Value>| v.and_then(Value::as_str).map(String::from);
    let food = v.get("food").filter(|f| !f.is_null());
    let unit = v.get("unit").filter(|u| !u.is_null());
    MealieItem {
        food: s(food.and_then(|f| f.get("name"))),
        quantity: v.get("quantity").and_then(Value::as_f64),
        unit_name: s(unit.and_then(|u| u.get("name"))),
        unit_abbreviation: s(unit.and_then(|u| u.get("abbreviation"))),
        use_abbreviation: unit
            .and_then(|u| u.get("useAbbreviation"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        note: s(v.get("note")),
        display: s(v.get("display")),
        label: s(v.get("label").and_then(|l| l.get("name"))).or_else(|| {
            s(food
                .and_then(|f| f.get("label"))
                .and_then(|l| l.get("name")))
        }),
    }
}
