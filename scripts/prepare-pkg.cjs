"use strict";

const fs = require("node:fs");
const path = require("node:path");

const generatedIgnore = path.join(__dirname, "..", "pkg", ".gitignore");

if (fs.existsSync(generatedIgnore)) {
  fs.unlinkSync(generatedIgnore);
}
