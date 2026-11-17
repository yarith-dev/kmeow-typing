//! Test harness only: I/O stays outside the portable library.
use khmer_engine::{ApprovedDataset, Engine, SuggestionFeatures, TypingContext, RankingRequest};
use serde::Deserialize;
use std::io::{self, Read};
#[derive(Deserialize)]
struct Request {
    dataset: Option<ApprovedDataset>,
    binary: Option<Vec<u8>>,
    app_data: Option<AppData>,
    #[serde(default)]
    inputs: Vec<String>,
    browse: Option<Vec<serde_json::Value>>,
    word_metadata: Option<serde_json::Value>,
    word_ranking: Option<serde_json::Value>,
    typing_overrides: Option<serde_json::Value>,
    metadata_queries: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    suggest: bool,
    #[serde(default)]
    features: SuggestionFeatures,
    #[serde(default)]
    profanity: bool,
    language: Option<String>,
    context: Option<TypingContext>,
    extension: Option<ApprovedDataset>,
    #[serde(default)]
    convert_extension: bool,
    ranking: Option<RankingRequest>,
    ranking_vocabulary: Option<String>,
}
#[derive(Deserialize)]
struct AppData { khmer: Vec<u8>, english: Vec<u8> }
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut data = String::new();
    io::stdin().read_to_string(&mut data)?;
    let request: Request = serde_json::from_str(&data)?;
    if request.profanity {
        let results = request.inputs.iter().map(|input| {
            khmer_engine::profanity::check_profanity_json(input, request.language.as_deref().unwrap_or("auto"))
                .map(|json| serde_json::from_str::<serde_json::Value>(&json).unwrap())
        }).collect::<Result<Vec<_>, _>>()?;
        println!("{}", serde_json::to_string(&results)?);
        return Ok(());
    }
    let mut engine = if let Some(files) = request.app_data {
        if request.dataset.is_some() || request.binary.is_some() { return Err("Supply only app data".into()); }
        Engine::from_app_data(&files.khmer, &files.english)?
    } else { match (request.dataset, request.binary) {
        (Some(dataset), None) => Engine::new(dataset)?,
        (None, Some(bytes)) => Engine::from_binary(bytes)?,
        _ => return Err("Supply exactly one dataset or binary fixture".into()),
    }};
    if let Some(files) = request.word_metadata {
        engine.load_word_metadata(&files["khmer"].to_string(), &files["english"].to_string())?;
    }
    if let Some(ranking) = request.word_ranking {
        engine.load_word_ranking_json(&ranking.to_string())?;
    }
    if let Some(language) = request.ranking_vocabulary {
        println!("{}", engine.ranking_vocabulary_json(&language)?);
        return Ok(());
    }
    if let Some(queries) = request.metadata_queries {
        let results = queries.iter().map(|query| {
            let result = if let Some(word) = query.get("word") {
                if query["translationsOnly"].as_bool().unwrap_or(false) {
                    engine.lookup_word_translations_json(query["language"].as_str().unwrap(), word.as_str().unwrap())
                } else { engine.lookup_word_metadata_json(query["language"].as_str().unwrap(), word.as_str().unwrap()) }
            } else { engine.browse_word_metadata_json(&query.to_string()) }?;
            Ok(serde_json::from_str::<serde_json::Value>(&result)?)
        }).collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
        println!("{}", serde_json::to_string(&results)?);
        return Ok(());
    }
    if let Some(requests) = request.browse {
        let results = requests.iter().map(|value| engine.browse_words_json(&value.to_string())
            .map(|json| serde_json::from_str::<serde_json::Value>(&json).unwrap()))
            .collect::<Result<Vec<_>, _>>()?;
        println!("{}", serde_json::to_string(&results)?);
        return Ok(());
    }
    let mut extension = request.extension.map(Engine::new).transpose()?;
    if let Some(settings) = request.typing_overrides {
        engine = engine.with_typing_overrides_json(&settings.to_string())?;
        extension = extension.map(|engine| engine.with_typing_overrides_json(&settings.to_string())).transpose()?;
    }
    let results: Vec<_> = request
        .inputs
        .iter()
        .map(|input| {
            let json = if let Some(ranking) = &request.ranking {
                if let Some(extension) = &extension {
                    engine.suggest_with_extension_ranking_json(extension, input, request.features,
                        &request.context.clone().unwrap_or_default(), ranking)
                } else { engine.suggest_with_ranking_json(input, request.features,
                    &request.context.clone().unwrap_or_default(), ranking) }
            } else if let Some(extension) = &extension {
                if request.convert_extension { engine.convert_with_extension_json(extension, input) }
                else { engine.suggest_with_extension_json(extension, input, request.features,
                    &request.context.clone().unwrap_or_default()) }
            } else if let Some(context) = &request.context {
                engine.suggest_with_context_json(input, request.features, context)
            } else if request.suggest {
                engine.suggest_with_features_json(input, request.features)
            } else {
                engine.convert_json(input)
            }?;
            Ok(serde_json::from_str::<serde_json::Value>(&json)?)
        })
        .collect::<Result<_, Box<dyn std::error::Error>>>()?;
    println!("{}", serde_json::to_string(&results)?);
    Ok(())
}
