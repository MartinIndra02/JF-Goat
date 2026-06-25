const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const tauriConfPath = path.resolve(__dirname, '../tauri.conf.json');
const externalLibsDir = path.resolve(__dirname, '../external-libs');
const targetBinary = path.resolve(__dirname, '../target/release/jfgoat');

function main() {
  if (process.platform !== 'darwin') {
    console.log('[before-bundle] Not macOS, skipping dylib bundling.');
    return;
  }

  console.log('[before-bundle] Running macOS dylib bundler...');

  // 1. Clean and create external-libs folder
  if (fs.existsSync(externalLibsDir)) {
    fs.rmSync(externalLibsDir, { recursive: true, force: true });
  }
  fs.mkdirSync(externalLibsDir, { recursive: true });

  // 2. Find libmpv path using brew or defaults
  let brewPrefix = '';
  try {
    brewPrefix = execSync('brew --prefix mpv', { encoding: 'utf-8' }).trim();
  } catch (e) {
    console.log('[before-bundle] brew --prefix mpv failed, checking common paths...');
  }

  let mpvDylib = '';
  const searchPaths = [
    brewPrefix ? path.join(brewPrefix, 'lib/libmpv.dylib') : '',
    '/opt/homebrew/lib/libmpv.dylib',
    '/usr/local/lib/libmpv.dylib'
  ].filter(Boolean);

  for (const p of searchPaths) {
    if (fs.existsSync(p)) {
      mpvDylib = p;
      break;
    }
  }

  if (!mpvDylib) {
    console.error('[before-bundle] Error: libmpv.dylib not found. Please run: brew install mpv');
    process.exit(1);
  }

  console.log(`[before-bundle] Found libmpv.dylib at: ${mpvDylib}`);

  // Copy libmpv.dylib to external-libs first so dylibbundler can analyze the binary which is linked to it
  const destMpv = path.join(externalLibsDir, 'libmpv.dylib');
  fs.copyFileSync(mpvDylib, destMpv);

  // 3. Run dylibbundler on the compiled binary
  try {
    const cmd = `dylibbundler -of -cd -p "@executable_path/../Frameworks/" -b -x "${targetBinary}" -d "${externalLibsDir}"`;
    console.log(`[before-bundle] Executing: ${cmd}`);
    execSync(cmd, { stdio: 'inherit' });
  } catch (e) {
    console.error('[before-bundle] dylibbundler failed:', e.message);
    process.exit(1);
  }

  // 4. Read all copied dylib files
  const dylibs = fs.readdirSync(externalLibsDir)
    .filter(file => file.endsWith('.dylib'))
    .map(file => `./external-libs/${file}`);

  console.log('[before-bundle] Found dylibs to bundle:', dylibs);

  // 5. Update tauri.conf.json
  const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, 'utf-8'));
  
  if (!tauriConf.bundle.macOS) {
    tauriConf.bundle.macOS = {};
  }
  tauriConf.bundle.macOS.frameworks = dylibs;

  fs.writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2), 'utf-8');
  console.log('[before-bundle] tauri.conf.json updated successfully.');
}

main();
