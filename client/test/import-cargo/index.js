const path = require("path");
const fs = require("fs");
const glob = require("glob");
const semver = require("semver");
const msgpack = require("msgpack5")();

const p = path.resolve(process.env.HOME, ".cargo", "registry", "index", "*", "*", "*", "*");

function normalisePkg(name) {
  return "test/" + name.replace("-", "_");
}

function desugar(wat) {
  const r = wat.replace(",", " ");
  if (/^[0-9.]+[*xX]$/.exec(r)) {
    return semver.validRange(r);
  }
  if (/^[0-9.]+[*xX]\.[*xX]$/.exec(r)) {
    return semver.validRange(r);
  }
  const m = /^= *([0-9a-zA-Z.-]+)$/.exec(r);
  if (m) {
    return m[1];
  }
  const m2 = /^> *([0-9.]+)$/.exec(r);
  if (m2) {
    return `^${m2[1]}`;
  }
  const m3 = /^\^([0-9.]+),? *>= *([0-9.]+)$/.exec(r);
  if (m3) {
    return `^${m3[2]}`;
  }
  const m4 = /^\^([0-9.]+),? *< *([0-9.]+)$/.exec(r);
  if (m4) {
    return `>= ${m4[1]} < ${m4[2]}`;
  }
  const m5 = /^\^([0-9.]+),? *<= *([0-9.]+)$/.exec(r);
  if (m5) {
    return `${m5[1]}`;
  }
  const m6 = /^>= *([0-9.]+),? *<= *([0-9.]+)$/.exec(r);
  if (m6) {
    return `>= ${m6[1]} < ${m6[2]}`;
  }
  const m7 = /^\^ *([0-9.]+),? *\^ *([0-9.]+)$/.exec(r);
  if (m7) {
    return `^${m7[1]}`;
  }
  const m8 = /^> *([0-9.]+),? *< *([0-9.]+)$/.exec(r);
  if (m8) {
    return `>= ${m8[1]} < ${m8[2]}`;
  }
  const m9 = /^>= *([0-9.]+),? *([0-9.]+\.[*xX])$/.exec(r);
  if (m9) {
    return `^${m9[1]}`;
  }
  return r;
}

function readFile(fn) {
  const c = fs.readFileSync(fn, "utf-8").split("\n").filter((s) => s.trim().length !== 0).map(JSON.parse);
  const reg = {};
  for (const entry of c) {
    const pkg = reg[normalisePkg(entry.name)] || {};
    const ver = entry.vers.replace(/\+[0-9a-zA-Z]+/, "");
    pkg[ver] = entry.deps.reduce((a, n) => {
      if (n.kind === "normal" && !n.optional) {
        a[normalisePkg(n.name)] = desugar(n.req);
      }
      return a;
    }, {});
    reg[normalisePkg(entry.name)] = pkg;
  }
  return reg;
}

glob(p, (err, files) => {
  if (err) throw err;
  const reg = files.map(readFile).reduce((a, n) => {
    for (const key in n) {
      a[key] = n[key];
    }
    return a;
  }, {});
  if (process.argv[2] === "--json") {
    process.stdout.write(JSON.stringify(reg, null, "  "));
  } else {
    process.stdout.write(msgpack.encode(reg));
  }
});