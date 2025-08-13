use candle_core::Device;
use e5_embeddings::Embeddings;

fn main() -> Result<()> {
    let device = Device::Cpu; // or Device::new_cuda(0)?
    let model = Embeddings::new("intfloat/multilingual-e5-base", device)?;

    let queries = vec![
        "query: how much protein should a female eat",
        "query: 南瓜的家常做法",
    ];
    let passages = vec![
        "passage: The CDC recommends 46g protein/day for women.",
        "passage: 清炒南瓜丝 做法: 南瓜削皮切丝, 快速翻炒一分钟...",
    ];

    let query_embs = model.embed(&queries)?;
    let passage_embs = model.embed(&passages)?;

    let sims = model.similarity(&query_embs, &passage_embs);
    println!("{sims:?}");

    Ok(())
}
