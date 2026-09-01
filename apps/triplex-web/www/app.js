import init, {
  generate_contract,
  list_recipe_columns,
  anonymize,
  check_pattern_conformance,
} from "./pkg/triplex_web.js";

await init();

function extensionOf(file) {
  const parts = file.name.split(".");
  return parts.length > 1 ? parts.pop().toLowerCase() : "";
}

function readBytes(file) {
  return file.arrayBuffer().then((buf) => new Uint8Array(buf));
}

function downloadBytes(bytes, filename, mime) {
  const blob = new Blob([bytes], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

function showError(el, err) {
  el.textContent = err instanceof Error ? err.message : String(err);
}

// --- 1. Generate contract ---

const contractFileInput = document.getElementById("contract-file");
const contractYamlBox = document.getElementById("contract-yaml");
const contractError = document.getElementById("contract-error");

document.getElementById("generate-contract-btn").addEventListener("click", async () => {
  contractError.textContent = "";
  const file = contractFileInput.files[0];
  if (!file) {
    contractError.textContent = "choose a file first";
    return;
  }
  try {
    const bytes = await readBytes(file);
    contractYamlBox.value = generate_contract(bytes, extensionOf(file));
  } catch (err) {
    showError(contractError, err);
  }
});

document.getElementById("download-contract-btn").addEventListener("click", () => {
  downloadBytes(new TextEncoder().encode(contractYamlBox.value), "contract.triplex.yaml", "text/yaml");
});

document.getElementById("copy-contract-to-check-btn").addEventListener("click", () => {
  document.getElementById("pattern-contract-yaml").value = contractYamlBox.value;
});

// --- 2. Anonymize ---

const anonymizeFileInput = document.getElementById("anonymize-file");
const recipeYamlBox = document.getElementById("recipe-yaml");
const saltInputs = document.getElementById("salt-inputs");
const anonymizeError = document.getElementById("anonymize-error");
const anonymizeReport = document.getElementById("anonymize-report");
const anonymizeWarning = document.getElementById("anonymize-warning");
const downloadMaskedBtn = document.getElementById("download-masked-btn");
const downloadMappingBtn = document.getElementById("download-mapping-btn");

let lastMaskedCsv = null;
let lastMappingCsv = null;

document.getElementById("load-recipe-columns-btn").addEventListener("click", () => {
  anonymizeError.textContent = "";
  saltInputs.innerHTML = "";
  let columns;
  try {
    columns = JSON.parse(list_recipe_columns(recipeYamlBox.value));
  } catch (err) {
    showError(anonymizeError, err);
    return;
  }
  for (const name of columns) {
    const label = document.createElement("label");
    label.textContent = `salt for "${name}": `;
    const input = document.createElement("input");
    input.type = "password";
    input.dataset.column = name;
    label.appendChild(input);
    saltInputs.appendChild(label);
  }
});

document.getElementById("anonymize-btn").addEventListener("click", async () => {
  anonymizeError.textContent = "";
  anonymizeReport.textContent = "";
  anonymizeWarning.textContent = "";
  downloadMaskedBtn.disabled = true;
  downloadMappingBtn.disabled = true;

  const file = anonymizeFileInput.files[0];
  if (!file) {
    anonymizeError.textContent = "choose a data file first";
    return;
  }

  const salts = {};
  for (const input of saltInputs.querySelectorAll("input[data-column]")) {
    salts[input.dataset.column] = input.value;
  }

  try {
    const bytes = await readBytes(file);
    const result = anonymize(bytes, extensionOf(file), recipeYamlBox.value, JSON.stringify(salts));
    const report = JSON.parse(result.report_json);
    anonymizeReport.textContent = JSON.stringify(report, null, 2);
    anonymizeWarning.textContent = report.warning;

    lastMaskedCsv = result.masked_csv;
    lastMappingCsv = result.mapping_csv;
    downloadMaskedBtn.disabled = false;
    downloadMappingBtn.disabled = false;
  } catch (err) {
    showError(anonymizeError, err);
  }
});

downloadMaskedBtn.addEventListener("click", () => {
  if (lastMaskedCsv) downloadBytes(lastMaskedCsv, "masked.csv", "text/csv");
});
downloadMappingBtn.addEventListener("click", () => {
  if (lastMappingCsv) downloadBytes(lastMappingCsv, "mapping.csv", "text/csv");
});

// --- 3. Check pattern conformance ---

const patternFileInput = document.getElementById("pattern-check-file");
const patternContractBox = document.getElementById("pattern-contract-yaml");
const patternError = document.getElementById("pattern-error");
const patternReport = document.getElementById("pattern-report");

document.getElementById("check-patterns-btn").addEventListener("click", async () => {
  patternError.textContent = "";
  patternReport.textContent = "";
  const file = patternFileInput.files[0];
  if (!file) {
    patternError.textContent = "choose a file first";
    return;
  }
  try {
    const bytes = await readBytes(file);
    const report = check_pattern_conformance(bytes, extensionOf(file), patternContractBox.value);
    patternReport.textContent = JSON.stringify(JSON.parse(report), null, 2);
  } catch (err) {
    showError(patternError, err);
  }
});
