use std::str::FromStr;

use redis::FromRedisValue;
use serde::{de, Deserialize, Deserializer, Serialize};

#[derive(Debug, Serialize, Clone)]
pub enum Edition {
    English,
    Spanish,
    French,
}

impl FromStr for Edition {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "english" => Ok(Edition::English),
            "spanish" => Ok(Edition::Spanish),
            "french" => Ok(Edition::French),
            e => Err(format!("Unknown edition: {}", e)),
        }
    }
}

impl<'de> de::Deserialize<'de> for Edition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

#[derive(Debug, Serialize, Clone)]
pub enum InventoryStatus {
    OnLoan,
    Available,
    Maintenance,
}

impl FromStr for InventoryStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "on_loan" => Ok(InventoryStatus::OnLoan),
            "onloan" => Ok(InventoryStatus::OnLoan), // need this for reading from redis
            "available" => Ok(InventoryStatus::Available),
            "maintenance" => Ok(InventoryStatus::Maintenance),
            e => Err(format!("Unknown inventory status: {}", e)),
        }
    }
}

impl<'de> de::Deserialize<'de> for InventoryStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Inventory {
    pub status: InventoryStatus,
    pub stock_id: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Metrics {
    pub rating_votes: u32,
    pub score: f32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Book {
    pub author: String,
    pub id: String,
    pub description: String,
    pub embedding: Option<Vec<f32>>,
    pub editions: Vec<Edition>,
    pub genres: Vec<String>,
    pub inventory: Vec<Inventory>,
    pub metrics: Metrics,
    pub pages: u32,
    pub title: String,
    pub url: String,
    pub year_published: u16,
}

impl FromRedisValue for Book {
    fn from_redis_value(v: &redis::Value) -> redis::RedisResult<Self> {
        let json: String = FromRedisValue::from_redis_value(v)?;
        let books: Vec<Book> = serde_json::from_str(&json)?;

        Ok(books.first().unwrap().clone())
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Recommendation {
    pub id: String,
    pub score: f32,
    pub title: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Recommendations {
    pub count: u64,
    pub recommendations: Vec<Recommendation>,
}

impl FromRedisValue for Recommendations {
    fn from_redis_value(v: &redis::Value) -> redis::RedisResult<Self> {
        let result = v.as_sequence() .unwrap();

        let mut iter = result.iter();
        let mut recommendations = Vec::new();

        let count: u64 = redis::from_redis_value(iter.next().unwrap())?;

        while let (Some(id), Some(values)) = (iter.next(), iter.next()) {
            let id: String = redis::from_redis_value(id)?;
            let mut values = values.as_sequence().unwrap().iter();
            let mut title: String = String::new();
            let mut score: f32 = 0.0;

            while let (Some(k), Some(v)) = (values.next(), values.next()) {
                let key: String = redis::from_redis_value(k)?;
                match key.as_str() {
                    "title" => {
                        title = redis::from_redis_value(v)?;
                    }
                    "score" => {
                        score = redis::from_redis_value(v)?;
                    }
                    _ => {}
                }
            }
            recommendations.push(Recommendation { id, title, score });
        }

        Ok(Recommendations {
            count,
            recommendations,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs::{self, File},
        io::BufReader,
    };

    use super::*;

    #[test]
    fn parse_book() {
        let file = File::open("../../data/books/1.json").unwrap();
        let reader = BufReader::new(file);
        let book: Book = serde_json::from_reader(reader).unwrap();
        dbg!(book);
    }

    #[test]
    fn parse_books() {
        let books = fs::read_dir("../../data/books").unwrap();
        for path in books {
            let file = File::open(path.unwrap().path()).unwrap();
            let reader = BufReader::new(file);
            let book: Book = serde_json::from_reader(reader).unwrap();
            dbg!(book);
        }
    }
}
