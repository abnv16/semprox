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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match_hit() {
        let cache = SemanticCache::new(10, 0.90);
        let embedding = vec![1.0, 0.0, 0.0];

        cache.insert(
            "gpt-4o-mini".to_string(),
            "Explain Kubernetes".to_string(),
            embedding.clone(),
            "Kubernetes is a container orchestration tool.".to_string(),
        );

        let hit = cache.lookup("gpt-4o-mini", &embedding);
        assert!(hit.is_some());
        let hit = hit.unwrap();
        assert_eq!(hit.content, "Kubernetes is a container orchestration tool.");
        assert!((hit.similarity - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_similarity_threshold() {
        let cache = SemanticCache::new(10, 0.90);
        cache.insert(
            "gpt-4o-mini".to_string(),
            "Prompt 1".to_string(),
            vec![1.0, 0.0, 0.0],
            "Answer 1".to_string(),
        );

        // Vector orthogonal (sim = 0.0) -> miss
        let miss = cache.lookup("gpt-4o-mini", &[0.0, 1.0, 0.0]);
        assert!(miss.is_none());

        // Vector with sim ~ 0.95 -> hit
        let hit = cache.lookup("gpt-4o-mini", &[0.95, 0.1, 0.0]);
        assert!(hit.is_some());
    }

    #[test]
    fn test_model_isolation() {
        let cache = SemanticCache::new(10, 0.90);
        let embedding = vec![1.0, 0.0, 0.0];

        cache.insert(
            "gpt-4o".to_string(),
            "Prompt".to_string(),
            embedding.clone(),
            "GPT-4 Answer".to_string(),
        );

        // Querying with different model shouldn't hit
        let miss = cache.lookup("gpt-4o-mini", &embedding);
        assert!(miss.is_none());

        // Querying with exact model hits
        let hit = cache.lookup("gpt-4o", &embedding);
        assert!(hit.is_some());
    }

    #[test]
    fn test_lru_eviction() {
        let cache = SemanticCache::new(2, 0.90);

        cache.insert(
            "model".to_string(),
            "prompt1".to_string(),
            vec![1.0, 0.0, 0.0],
            "answer1".to_string(),
        );
        std::thread::sleep(std::time::Duration::from_millis(10));

        cache.insert(
            "model".to_string(),
            "prompt2".to_string(),
            vec![0.0, 1.0, 0.0],
            "answer2".to_string(),
        );

        assert_eq!(cache.size(), 2);

        // Access prompt1 to make prompt2 the oldest
        let _ = cache.lookup("model", &[1.0, 0.0, 0.0]);
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Insert third entry -> should evict prompt2
        cache.insert(
            "model".to_string(),
            "prompt3".to_string(),
            vec![0.0, 0.0, 1.0],
            "answer3".to_string(),
        );

        assert_eq!(cache.size(), 2);
        assert!(cache.lookup("model", &[1.0, 0.0, 0.0]).is_some());
        assert!(cache.lookup("model", &[0.0, 1.0, 0.0]).is_none());
        assert!(cache.lookup("model", &[0.0, 0.0, 1.0]).is_some());
    }
}
