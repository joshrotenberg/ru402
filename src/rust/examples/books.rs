use std::{
    fs::{self, File},
    io::BufReader,
};

use anyhow::Result;
use byteorder::{LittleEndian, WriteBytesExt};
use clap::Parser;
use redis::{JsonCommands, Value};
use ru402::book::{Book, Recommendations};
use rust_bert::pipelines::sentence_embeddings::{
    SentenceEmbeddingsBuilder, SentenceEmbeddingsModelType,
};

const INDEX_NAME: &str = "idx:books";

#[derive(Debug, Parser)]
struct Cli {
    /// The redis url
    #[clap(short, long, default_value = "redis://127.0.0.1:6379")]
    redis_url: String,
    /// The id of the book to get recommendations for
    #[clap(short, long, default_value = "")]
    id: String,
    /// Create the index (set to false to skip creating the index and just query the data)
    #[clap(short, long, action = clap::ArgAction::Set)]
    index: bool,
    /// Load the data (set to false to skip loading the data and just query the index)
    #[clap(short, long, action = clap::ArgAction::Set)]
    load: bool,
}

fn main() -> Result<()> {
    let args = Cli::parse();

    // get a redis connection
    let client = redis::Client::open(args.redis_url)?;
    let mut connection = client.get_connection()?;

    // define the model
    let model = SentenceEmbeddingsBuilder::remote(SentenceEmbeddingsModelType::AllMiniLmL6V2)
        .create_model()?;

    // read the books and store them as json documents, generating and adding in the embeddings for the description
    if args.load {
        for file in fs::read_dir("../../data/books")? {
            let file = file?;
            let file = File::open(file.path())?;
            let reader = BufReader::new(file);
            let mut book: Book = serde_json::from_reader(reader).unwrap();
            let key = format!("book:{}", book.id);

            let embedding = model.encode(&[&book.description])?;
            book.embedding = Some(embedding[0].to_vec());
            connection.json_set(key, "$", &book)?;
        }
    }

    // create the index
    if args.index {
        create_index(&mut connection)?;
    }

    // run sample queries if no id was specified
    if args.id.is_empty() {
        println!("Recommendations for book:26415");
        print_recommendations(get_recommendation(&mut connection, "book:26415")?);
        println!("Recommendations for book:9");
        print_recommendations(get_recommendation(&mut connection, "book:9")?);

        println!("Recommendations by range for book:26415");
        print_recommendations(get_recommendation_by_range(&mut connection, "book:26415")?);
        println!("Recommendations by range for book:9");
        print_recommendations(get_recommendation_by_range(&mut connection, "book:9")?);
    } else {
        println!("Recommendations for {}", args.id);
        print_recommendations(get_recommendation(&mut connection, &args.id)?);

        println!("Recommendations by range for {}", args.id);
        print_recommendations(get_recommendation_by_range(&mut connection, &args.id)?);
    }

    Ok(())
}

// print the recommendations
fn print_recommendations(recommendations: Recommendations) {
    for r in &recommendations.recommendations {
        println!(
            "\tid: {}\n\ttitle: {}\n\tscore: {}\n\n",
            r.id, r.title, r.score
        );
    }
}

// encode the embeddings as a byte array
fn encode(fs: Vec<f32>) -> Vec<u8> {
    let mut vec: Vec<u8> = Vec::new();
    for f in fs {
        vec.write_f32::<LittleEndian>(f).unwrap();
    }
    vec
}

// create the index if it doesn't exist
fn create_index(connection: &mut redis::Connection) -> Result<()> {
    let result: Result<Value, _> = redis::cmd("FT._LIST").query(connection);
    if let Ok(Value::Bulk(ref values)) = result {
        if values
            .iter()
            .any(|v| v == &Value::Status(String::from(INDEX_NAME)))
        {
            return Ok(());
        }
    }

    let _ = redis::cmd("FT.CREATE")
        .arg(INDEX_NAME)
        .arg("ON")
        .arg("JSON")
        .arg("PREFIX")
        .arg("1")
        .arg("book:")
        .arg("SCHEMA")
        // author
        .arg("$.author")
        .arg("AS")
        .arg("author")
        .arg("TEXT")
        // title
        .arg("$.title")
        .arg("AS")
        .arg("title")
        .arg("TEXT")
        // description
        .arg("$.description")
        .arg("AS")
        .arg("description")
        .arg("TEXT")
        // embedding
        .arg("$.embedding")
        .arg("AS")
        .arg("embedding")
        .arg("VECTOR")
        // search parameters
        .arg("HNSW")
        .arg("6")
        .arg("TYPE")
        .arg("FLOAT32")
        .arg("DIM")
        .arg("384")
        .arg("DISTANCE_METRIC")
        .arg("COSINE")
        .query(connection)?;
    Ok(())
}

// get the recommendations for a book
fn get_recommendation(connection: &mut redis::Connection, key: &str) -> Result<Recommendations> {
    let book: Book = connection.json_get(key, "$")?;

    if let Some(embedding) = book.embedding {
        let encoded_embedding = encode(embedding);
        let query = "*=>[KNN 5 @embedding $vec AS score]";
        let recommendations: Recommendations = redis::cmd("FT.SEARCH")
            .arg(INDEX_NAME)
            .arg(query)
            .arg("PARAMS")
            .arg(2)
            .arg("vec")
            .arg(encoded_embedding)
            .arg("RETURN")
            .arg("2")
            .arg("title")
            .arg("score")
            .arg("SORTBY")
            .arg("score")
            .arg("LIMIT")
            .arg(0)
            .arg(5)
            .arg("DIALECT")
            .arg("2")
            .query(connection)?;

        return Ok(recommendations);
    }
    anyhow::bail!("No embedding found for book {}", key);
}

// get the recommendations for a book by range
fn get_recommendation_by_range(
    connection: &mut redis::Connection,
    key: &str,
) -> Result<Recommendations> {
    let book: Book = connection.json_get(key, "$")?;

    if let Some(embedding) = book.embedding {
        let encoded_embedding = encode(embedding);
        let query = "@embedding:[VECTOR_RANGE $radius $vec]=>{$YIELD_DISTANCE_AS: score}";

        let recommendations: Recommendations = redis::cmd("FT.SEARCH")
            .arg(INDEX_NAME)
            .arg(query)
            .arg("PARAMS")
            .arg(4)
            .arg("radius")
            .arg(3)
            .arg("vec")
            .arg(encoded_embedding)
            .arg("RETURN")
            .arg(2)
            .arg("title")
            .arg("score")
            .arg("SORTBY")
            .arg("score")
            .arg("LIMIT")
            .arg(0)
            .arg(5)
            .arg("DIALECT")
            .arg("2")
            .query(connection)?;

        return Ok(recommendations);
    }
    anyhow::bail!("No embedding found for book {}", key);
}
