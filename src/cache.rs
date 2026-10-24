use parking_lot::RwLock;
use std::time::Instant;

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct CacheEntry {
    pub id: String,
    pub model: String,
    pub prompt: String,
    pub embedding: Vec<f32>,
    pub content: String,
    pub last_accessed: Instant,
}

#[derive(Clone, Debug)]
pub struct CacheHit {
    pub content: String,
    pub similarity: f32,
    pub matched_prompt: String,
}

pub struct SemanticCache {
    max_size: usize,
    similarity_threshold: f32,
    entries: RwLock<Vec<CacheEntry>>,
}

fn normalize(vec: &mut [f32]) {
    let norm_sq: f32 = vec.iter().map(|x| x * x).sum();
    let norm = norm_sq.sqrt();
    if norm > 1e-9 {
        let inv_norm = 1.0 / norm;
        for x in vec.iter_mut() {
            *x *= inv_norm;
        }
    }
}

fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

impl SemanticCache {
    pub fn new(max_size: usize, similarity_threshold: f32) -> Self {
        Self {
            max_size,
            similarity_threshold,
            entries: RwLock::new(Vec::new()),
        }
    }

    pub fn lookup(&self, model: &str, query_embedding: &[f32]) -> Option<CacheHit> {
        let mut norm_query = query_embedding.to_vec();
        normalize(&mut norm_query);

        let entries = self.entries.read();
        let mut best_idx: Option<usize> = None;
        let mut best_sim = -1.0f32;

        for (idx, entry) in entries.iter().enumerate() {
            if entry.model != model {
                continue;
            }
            let sim = dot_product(&norm_query, &entry.embedding);
            if sim > best_sim {
                best_sim = sim;
                best_idx = Some(idx);
            }
        }

        if let Some(idx) = best_idx {
            if best_sim >= self.similarity_threshold {
                let hit = CacheHit {
                    content: entries[idx].content.clone(),
                    similarity: best_sim,
                    matched_prompt: entries[idx].prompt.clone(),
                };
                drop(entries);

                let mut entries_mut = self.entries.write();
                if let Some(entry) = entries_mut.get_mut(idx) {
                    entry.last_accessed = Instant::now();
                }

                return Some(hit);
            }
        }

        None
    }

    pub fn insert(&self, model: String, prompt: String, mut embedding: Vec<f32>, content: String) {
        if content.trim().is_empty() {
            return;
        }
        normalize(&mut embedding);

        let mut entries = self.entries.write();

        if entries.len() >= self.max_size && !entries.is_empty() {
            if let Some((oldest_idx, _)) = entries
                .iter()
                .enumerate()
                .min_by_key(|(_, entry)| entry.last_accessed)
            {
                entries.swap_remove(oldest_idx);
            }
        }

        let id = format!("cache_{}", entries.len() + 1);
        entries.push(CacheEntry {
            id,
            model,
            prompt,
            embedding,
            content,
            last_accessed: Instant::now(),
        });
    }

    pub fn size(&self) -> usize {
        self.entries.read().len()
    }

    #[allow(dead_code)]
    pub fn clear(&self) {
        self.entries.write().clear();
    }
}
