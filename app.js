/**
 * semprox - Interactive Demo & Documentation Logic
 */

// Initial Seed Cache Data
let cacheEntries = [
  {
    id: "cache_1",
    model: "gpt-4o-mini",
    prompt: "Explain Kubernetes in one sentence",
    answer: "Kubernetes is an open-source orchestration system that automates the deployment, scaling, and management of containerized applications.",
    hits: 3,
    created: "Just now"
  },
  {
    id: "cache_2",
    model: "gpt-4o-mini",
    prompt: "What is Docker and how does it work?",
    answer: "Docker is a platform that uses OS-level virtualization to deliver software in lightweight packages called containers, isolating dependencies and configuration.",
    hits: 1,
    created: "2 mins ago"
  }
];

// Calculate character-level & word n-gram pseudo semantic similarity
function computeCosineSimilarity(query, target) {
  const qClean = query.toLowerCase().trim().replace(/[^\w\s]/g, "");
  const tClean = target.toLowerCase().trim().replace(/[^\w\s]/g, "");

  if (qClean === tClean) return 1.0;

  // Extract word sets & bi-grams for semantic resemblance
  const getTokens = (str) => {
    const words = str.split(/\s+/).filter(Boolean);
    const bigrams = [];
    for (let i = 0; i < words.length - 1; i++) {
      bigrams.push(words[i] + "_" + words[i + 1]);
    }
    return new Set([...words, ...bigrams]);
  };

  const setQ = getTokens(qClean);
  const setT = getTokens(tClean);

  if (setQ.size === 0 || setT.size === 0) return 0.0;

  let intersection = 0;
  setQ.forEach((item) => {
    if (setT.has(item)) intersection++;
  });

  // Jaccard similarity mapped to high cosine similarity range typical in embedding models
  const jaccard = intersection / (setQ.size + setT.size - intersection);
  
  // Dense embedding models produce cosine similarities typically between 0.70 and 0.98 for related queries
  let sim = 0.55 + (jaccard * 0.43);

  // Bonus for strong semantic overlap keywords
  const k8sTerms = ["kubernetes", "k8s", "container", "orchestration"];
  const countQ = k8sTerms.filter(t => qClean.includes(t)).length;
  const countT = k8sTerms.filter(t => tClean.includes(t)).length;
  if (countQ > 0 && countT > 0) {
    sim = Math.min(0.97, Math.max(sim, 0.92));
  }

  return Math.min(0.999, Math.max(0.20, sim));
}

// Render the in-memory cache table
function renderCacheTable() {
  const tbody = document.getElementById("cacheTableBody");
  if (!tbody) return;

  if (cacheEntries.length === 0) {
    tbody.innerHTML = `<tr><td colspan="4" style="text-align:center; color: var(--text-dim); padding: 1.5rem;">Cache is empty. Send a request to populate!</td></tr>`;
    return;
  }

  tbody.innerHTML = cacheEntries.map(entry => `
    <tr>
      <td style="font-family: var(--font-mono); color: var(--accent-cyan);">${entry.id}</td>
      <td style="max-width: 200px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;" title="${entry.prompt}">
        ${entry.prompt}
      </td>
      <td><span class="brand-badge">${entry.model}</span></td>
      <td style="font-family: var(--font-mono); color: var(--accent-emerald);">${entry.hits}</td>
    </tr>
  `).join("");
}

// Execute Interactive Request Simulation
let isSimulating = false;

async function executeSimulatedRequest() {
  if (isSimulating) return;

  const promptInput = document.getElementById("simPrompt");
  const thresholdSlider = document.getElementById("simThreshold");
  const streamToggle = document.getElementById("simStreamToggle");
  const responseText = document.getElementById("simResponseText");
  const statusBadge = document.getElementById("simStatusBadge");
  const latencyBadge = document.getElementById("simLatencyBadge");
  const costBadge = document.getElementById("simCostBadge");
  const simBtn = document.getElementById("simRunBtn");

  const prompt = promptInput.value.trim();
  if (!prompt) {
    showToast("Please enter a prompt to test!");
    return;
  }

  const threshold = parseFloat(thresholdSlider.value);
  const isStream = streamToggle.checked;

  isSimulating = true;
  simBtn.disabled = true;
  simBtn.innerHTML = `<span>⚡ Processing...</span>`;

  statusBadge.className = "status-badge idle";
  statusBadge.innerHTML = "Computing Embedding...";
  latencyBadge.innerHTML = "--";
  costBadge.innerHTML = "--";
  responseText.textContent = "Extracting prompt tokens -> Generating ONNX embedding (384 dimensions)...";

  // Simulate embedding generation latency (~20ms)
  await new Promise(r => setTimeout(r, 120));

  // Find best match in cache
  let bestEntry = null;
  let bestSim = 0.0;

  for (const entry of cacheEntries) {
    const sim = computeCosineSimilarity(prompt, entry.prompt);
    if (sim > bestSim) {
      bestSim = sim;
      bestEntry = entry;
    }
  }

  const isHit = bestEntry && bestSim >= threshold;

  if (isHit) {
    // CACHE HIT
    bestEntry.hits++;
    renderCacheTable();

    statusBadge.className = "status-badge hit";
    statusBadge.innerHTML = `🟢 HIT (Sim: ${bestSim.toFixed(4)})`;
    latencyBadge.innerHTML = `<strong>3.4ms</strong>`;
    costBadge.innerHTML = `<strong style="color:var(--accent-emerald);">$0.0000</strong>`;

    if (isStream) {
      responseText.textContent = "";
      const words = bestEntry.answer.split(" ");
      for (const word of words) {
        responseText.textContent += word + " ";
        await new Promise(r => setTimeout(r, 20));
      }
      responseText.textContent += "\n\n[Cache Hit: 0 upstream tokens consumed]";
    } else {
      responseText.textContent = JSON.stringify({
        id: `chatcmpl-cached-${bestEntry.id}`,
        object: "chat.completion",
        model: "gpt-4o-mini",
        choices: [{
          index: 0,
          message: { role: "assistant", content: bestEntry.answer },
          finish_reason: "stop"
        }],
        usage: { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 }
      }, null, 2);
    }
  } else {
    // CACHE MISS
    statusBadge.className = "status-badge miss";
    statusBadge.innerHTML = `🟠 MISS (Best: ${bestSim.toFixed(4)})`;
    responseText.textContent = "Cache miss. Forwarding request upstream to OpenAI (https://api.openai.com)...\nWaiting for response...";

    // Simulate upstream LLM generation latency (~1,100ms)
    await new Promise(r => setTimeout(r, 1100));

    latencyBadge.innerHTML = `<strong>1,180ms</strong>`;
    costBadge.innerHTML = `<strong style="color:var(--accent-amber);">$0.0015</strong>`;

    const generatedAnswer = `[Upstream Response] Here is a synthesized completion answering: "${prompt}".`;

    // Cache the new item
    const newId = `cache_${cacheEntries.length + 1}`;
    cacheEntries.unshift({
      id: newId,
      model: "gpt-4o-mini",
      prompt: prompt,
      answer: generatedAnswer,
      hits: 0,
      created: "Just now"
    });
    renderCacheTable();

    if (isStream) {
      responseText.textContent = "";
      const words = generatedAnswer.split(" ");
      for (const word of words) {
        responseText.textContent += word + " ";
        await new Promise(r => setTimeout(r, 35));
      }
      responseText.textContent += "\n\n[Cache Miss: Response automatically cached for subsequent queries!]";
    } else {
      responseText.textContent = JSON.stringify({
        id: "chatcmpl-upstream-live",
        object: "chat.completion",
        model: "gpt-4o-mini",
        choices: [{
          index: 0,
          message: { role: "assistant", content: generatedAnswer },
          finish_reason: "stop"
        }],
        usage: { prompt_tokens: 18, completion_tokens: 24, total_tokens: 42 }
      }, null, 2);
    }
  }

  isSimulating = false;
  simBtn.disabled = false;
  simBtn.innerHTML = `<span>⚡ Execute Request</span>`;
}

// Reset Simulator Cache
function resetSimulatorCache() {
  cacheEntries = [];
  renderCacheTable();
  const responseText = document.getElementById("simResponseText");
  const statusBadge = document.getElementById("simStatusBadge");
  const latencyBadge = document.getElementById("simLatencyBadge");
  const costBadge = document.getElementById("simCostBadge");

  if (responseText) responseText.textContent = "Cache cleared. Ready for your first request.";
  if (statusBadge) {
    statusBadge.className = "status-badge idle";
    statusBadge.innerHTML = "IDLE";
  }
  if (latencyBadge) latencyBadge.innerHTML = "--";
  if (costBadge) costBadge.innerHTML = "--";
  showToast("In-memory cache reset!");
}

// ROI & Cost Calculator Logic
function updateRoiCalculator() {
  const reqSlider = document.getElementById("calcRequests");
  const tokenSlider = document.getElementById("calcTokens");
  const hitRateSlider = document.getElementById("calcHitRate");

  const reqVal = parseInt(reqSlider.value, 10);
  const tokenVal = parseInt(tokenSlider.value, 10);
  const hitRateVal = parseInt(hitRateSlider.value, 10) / 100;

  document.getElementById("calcRequestsVal").textContent = reqVal.toLocaleString();
  document.getElementById("calcTokensVal").textContent = tokenVal.toLocaleString();
  document.getElementById("calcHitRateVal").textContent = Math.round(hitRateVal * 100) + "%";

  // Blended average cost per 1M tokens ($2.50 per 1M tokens)
  const costPerToken = 2.50 / 1_000_000;
  const monthlyTokens = reqVal * tokenVal;
  const monthlyTotalCost = monthlyTokens * costPerToken;

  const monthlySavings = monthlyTotalCost * hitRateVal;
  const yearlySavings = monthlySavings * 12;

  // Latency saved: ~1.2s per cache hit
  const cacheHitCount = reqVal * hitRateVal;
  const secondsSaved = cacheHitCount * 1.2;
  const hoursSaved = Math.round(secondsSaved / 3600);

  document.getElementById("calcSavedDollars").textContent = "$" + Math.round(monthlySavings).toLocaleString();
  document.getElementById("calcYearlyDollars").textContent = "$" + Math.round(yearlySavings).toLocaleString();
  document.getElementById("calcHoursSaved").textContent = hoursSaved.toLocaleString() + " hrs";
  document.getElementById("calcCallsAvoided").textContent = Math.round(cacheHitCount).toLocaleString();
}

// Code Tabs switching
const codeSnippets = {
  curl: `curl http://localhost:8080/v1/chat/completions \\
  -H "Authorization: Bearer $OPENAI_API_KEY" \\
  -H "Content-Type: application/json" \\
  -d '{
    "model": "gpt-4o-mini",
    "messages": [
      {"role": "user", "content": "Explain Kubernetes in one sentence"}
    ],
    "stream": true
  }'`,

  python: `from openai import OpenAI

# Simply redirect baseURL to semprox proxy
client = OpenAI(
    base_url="http://localhost:8080/v1",
    api_key="your-openai-api-key"
)

response = client.chat.completions.create(
    model="gpt-4o-mini",
    messages=[{"role": "user", "content": "Explain Kubernetes in one sentence"}],
    stream=True
)

for chunk in response:
    content = chunk.choices[0].delta.content or ""
    print(content, end="", flush=True)`,

  typescript: `import OpenAI from "openai";

const openai = new OpenAI({
  baseURL: "http://localhost:8080/v1",
  apiKey: process.env.OPENAI_API_KEY,
});

async function main() {
  const completion = await openai.chat.completions.create({
    model: "gpt-4o-mini",
    messages: [{ role: "user", content: "Explain Kubernetes in one sentence" }],
  });

  console.log(completion.choices[0].message.content);
}
main();`,

  docker: `version: '3.8'

services:
  semprox:
    image: semprox:latest
    ports:
      - "8080:8080"
    environment:
      - PROXY_PORT=8080
      - UPSTREAM_URL=https://api.openai.com
      - UPSTREAM_API_KEY=\${OPENAI_API_KEY}
      - SIMILARITY_THRESHOLD=0.90
      - MAX_CACHE_SIZE=10000
    volumes:
      - model_cache:/app/models

volumes:
  model_cache:`
};

function switchCodeTab(lang) {
  document.querySelectorAll(".code-tab-btn").forEach(btn => btn.classList.remove("active"));
  const activeBtn = document.querySelector(`.code-tab-btn[data-lang="${lang}"]`);
  if (activeBtn) activeBtn.classList.add("active");

  const pre = document.getElementById("quickstartCodePre");
  if (pre && codeSnippets[lang]) {
    pre.textContent = codeSnippets[lang];
  }
}

// Copy Code Helper
function copyCode() {
  const pre = document.getElementById("quickstartCodePre");
  if (!pre) return;
  navigator.clipboard.writeText(pre.textContent).then(() => {
    showToast("Code copied to clipboard!");
  }).catch(() => {
    showToast("Failed to copy code.");
  });
}

// Toast Display
let toastTimeout = null;
function showToast(msg) {
  const toast = document.getElementById("toastNotice");
  if (!toast) return;

  toast.textContent = msg;
  toast.classList.add("show");

  clearTimeout(toastTimeout);
  toastTimeout = setTimeout(() => {
    toast.classList.remove("show");
  }, 2500);
}

// DOM Initialization
document.addEventListener("DOMContentLoaded", () => {
  renderCacheTable();
  updateRoiCalculator();
  switchCodeTab("curl");

  // Threshold slider live indicator
  const thresholdSlider = document.getElementById("simThreshold");
  const thresholdBadge = document.getElementById("thresholdValBadge");
  if (thresholdSlider && thresholdBadge) {
    thresholdSlider.addEventListener("input", (e) => {
      thresholdBadge.textContent = parseFloat(e.target.value).toFixed(2);
    });
  }

  // Calculator sliders listeners
  ["calcRequests", "calcTokens", "calcHitRate"].forEach(id => {
    const el = document.getElementById(id);
    if (el) el.addEventListener("input", updateRoiCalculator);
  });

  // Sample Chips Click
  document.querySelectorAll(".sample-chip").forEach(chip => {
    chip.addEventListener("click", () => {
      const promptInput = document.getElementById("simPrompt");
      if (promptInput) {
        promptInput.value = chip.getAttribute("data-prompt") || chip.textContent.trim();
      }
    });
  });

  // Code tab buttons
  document.querySelectorAll(".code-tab-btn").forEach(btn => {
    btn.addEventListener("click", () => {
      switchCodeTab(btn.getAttribute("data-lang"));
    });
  });
});
