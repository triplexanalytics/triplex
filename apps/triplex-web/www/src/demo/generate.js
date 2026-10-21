// The demo's two sample datasets, generated from a fixed seed so the files in
// tests/fixtures/demo/ can be rebuilt byte for byte (`npm run demo-data`). Every row is made up:
// no real person is in either file. Postcodes are real four-digit areas, since a register
// without real geography would not show why a postcode counts as identifying.

/** A small seeded generator (mulberry32): same seed, same sequence, on every platform. */
function rng(seed) {
  let s = seed >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const pick = (r, weights) => {
  let x = r() * weights.reduce((sum, [, w]) => sum + w, 0);
  for (const [value, w] of weights) if ((x -= w) < 0) return value;
  return weights[weights.length - 1][0];
};
const uniform = (r, items) => items[Math.floor(r() * items.length)];
const normal = (r) => Math.sqrt(-2 * Math.log(1 - r())) * Math.cos(2 * Math.PI * r());
const clamp = (x, lo, hi) => Math.max(lo, Math.min(hi, x));
const pad = (n, width) => String(n).padStart(width, "0");

const MUNICIPALITIES = {
  Utrecht: [3511, 3521, 3531, 3541, 3551, 3561, 3572, 3581],
  Amersfoort: [3811, 3812, 3813, 3815, 3816, 3817, 3818, 3824],
  Zeist: [3701, 3702, 3703, 3704, 3705, 3707, 3708, 3709],
};
const FIRST_NAMES = {
  F: ["Anna", "Emma", "Fenna", "Julia", "Lisa", "Sanne", "Sophie", "Tess", "Fatima", "Elif"],
  M: ["Bram", "Daan", "Jan", "Lars", "Mark", "Noah", "Sem", "Thijs", "Mohammed", "Yusuf"],
  X: ["Alex", "Kim", "Robin", "Sam"],
};
const LAST_NAMES = [
  "de Jong",
  "Jansen",
  "de Vries",
  "van den Berg",
  "van Dijk",
  "Bakker",
  "Visser",
  "Smit",
  "Meijer",
  "de Boer",
  "Mulder",
  "Bos",
  "Vos",
  "Peters",
  "Hendriks",
  "Dekker",
  "Yilmaz",
  "El Amrani",
  "Kaya",
  "Brouwer",
];
const SCHEMES = [
  ["Participatiewet", 6],
  ["Bijzondere bijstand", 3],
  ["IOAW", 1],
];
const BASE_AMOUNT = { single: 1090, single_parent: 1400, couple: 1560, couple_with_children: 1560 };

/**
 * A municipal benefits register: who receives which scheme, where they live, how much. Three
 * columns that look harmless on their own (postcode, birth year, sex) single out most people
 * together, which is the story the demo opens with.
 * @returns {string} CSV text
 */
export function municipalCsv(rows = 1200, seed = 2026) {
  const r = rng(seed);
  const out = [
    "client_id,name,email,municipality,postcode,birth_year,sex,household,scheme,monthly_amount,start_date",
  ];
  for (let i = 0; i < rows; i++) {
    const id = 100001 + i;
    const municipality = pick(r, [
      ["Utrecht", 5],
      ["Amersfoort", 3],
      ["Zeist", 2],
    ]);
    const postcode = uniform(r, MUNICIPALITIES[municipality]);
    const birthYear = Math.round(clamp(1972 + 15 * normal(r), 1940, 2006));
    const sex = pick(r, [
      ["F", 52],
      ["M", 47],
      ["X", 1],
    ]);
    const household = pick(r, [
      ["single", 5],
      ["single_parent", 2],
      ["couple", 2],
      ["couple_with_children", 2],
    ]);
    const scheme = pick(r, SCHEMES);
    const base = scheme === "Bijzondere bijstand" ? 180 : BASE_AMOUNT[household];
    const amount = Math.max(25, base + 60 * normal(r));
    const start = new Date(Date.UTC(2019, 0, 1) + Math.floor(r() * 2800) * 86400000);
    out.push(
      [
        `K${id}`,
        `${uniform(r, FIRST_NAMES[sex])} ${uniform(r, LAST_NAMES)}`,
        `client${id}@example.nl`,
        municipality,
        postcode,
        birthYear,
        sex,
        household,
        scheme,
        amount.toFixed(2),
        start.toISOString().slice(0, 10),
      ].join(","),
    );
  }
  return out.join("\n") + "\n";
}

const PROVINCES = [
  ["Zuid-Holland", 21],
  ["Noord-Holland", 17],
  ["Noord-Brabant", 15],
  ["Gelderland", 12],
  ["Utrecht", 8],
  ["Overijssel", 7],
  ["Limburg", 6],
  ["Friesland", 4],
  ["Groningen", 3],
  ["Drenthe", 3],
  ["Flevoland", 2],
  ["Zeeland", 2],
];
const EDUCATION = [
  ["primary", 8, 24000],
  ["secondary", 30, 31000],
  ["vocational", 30, 36000],
  ["bachelor", 20, 47000],
  ["master", 11, 58000],
  ["doctorate", 1, 66000],
];
const OCCUPATION = [
  ["care", 34],
  ["education", 36],
  ["industry", 39],
  ["retail", 28],
  ["services", 37],
  ["public_administration", 37],
];

/**
 * Person-level microdata as a statistical office might hold it before a release. Income follows
 * education and age, and hours follow occupation, so a synthesizer has structure to keep. A few
 * combinations are rare (a doctorate in a small province), which is where record-level risk
 * shows.
 * @returns {string} CSV text
 */
export function statisticalCsv(rows = 2000, seed = 2027) {
  const r = rng(seed);
  const out = [
    "person_id,province,age,sex,education,occupation,hours_per_week,annual_income,household_size,born_abroad",
  ];
  for (let i = 0; i < rows; i++) {
    const age = Math.round(clamp(43 + 12 * normal(r), 18, 67));
    const sex = pick(r, [
      ["F", 50],
      ["M", 50],
    ]);
    const [education, , base] = pick(
      r,
      EDUCATION.map((row) => [row, row[1]]),
    );
    const [occupation, hours] = pick(
      r,
      OCCUPATION.map((row) => [row, 1]),
    );
    const worked = Math.round(clamp(hours + 6 * normal(r), 8, 48));
    const experience = 1 + 0.012 * (age - 18);
    const income = Math.max(9000, base * experience * (worked / 36) + 4000 * normal(r));
    out.push(
      [
        `P${pad(i + 1, 5)}`,
        pick(r, PROVINCES),
        age,
        sex,
        education,
        occupation,
        worked,
        Math.round(income / 100) * 100,
        pick(r, [
          [1, 3],
          [2, 4],
          [3, 2],
          [4, 2],
          [5, 1],
        ]),
        r() < 0.17 ? "yes" : "no",
      ].join(","),
    );
  }
  return out.join("\n") + "\n";
}
