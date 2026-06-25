import fs from 'fs';
import path from 'path';

// Paths
const tauriConfPath = path.resolve('src-tauri/tauri.conf.json');
const bundleDir = path.resolve('src-tauri/target/release/bundle');

function run() {
  if (!fs.existsSync(tauriConfPath)) {
    console.error(`Error: tauri.conf.json not found at ${tauriConfPath}`);
    process.exit(1);
  }

  const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, 'utf-8'));
  const version = tauriConf.version;
  if (!version) {
    console.error('Error: version not found in tauri.conf.json');
    process.exit(1);
  }

  console.log(`Generating latest.json for version ${version}...`);

  // Recursively find .sig files in bundleDir
  const sigFiles = findFiles(bundleDir, /\.sig$/);
  if (sigFiles.length === 0) {
    console.error(`Error: No .sig files found in ${bundleDir}`);
    process.exit(1);
  }

  // We want to prefer the NSIS setup exe signature if available
  let selectedSigFile = sigFiles.find(f => f.endsWith('-setup.exe.sig') || f.endsWith('.exe.sig'));
  if (!selectedSigFile) {
    // fallback to msi signature
    selectedSigFile = sigFiles.find(f => f.endsWith('.msi.sig'));
  }
  if (!selectedSigFile) {
    // fallback to any sig file
    selectedSigFile = sigFiles[0];
  }

  console.log(`Selected signature file: ${selectedSigFile}`);
  const signature = fs.readFileSync(selectedSigFile, 'utf-8').trim();

  // Find the corresponding installer file
  const installerFile = selectedSigFile.slice(0, -4); // remove .sig
  if (!fs.existsSync(installerFile)) {
    console.error(`Error: Corresponding installer file not found at ${installerFile}`);
    process.exit(1);
  }

  const installerFilename = path.basename(installerFile);
  console.log(`Installer filename: ${installerFilename}`);

  // Construct the download URL
  // We can use the actual repository name from tauri.conf.json updater endpoint, or hardcode/detect it.
  // The updater endpoints in tauri.conf.json are:
  // "https://github.com/MartinIndra02/jfFast/releases/latest/download/latest.json"
  // Let's extract the repository name from endpoints, but replace "jfFast" with "JF-Goat" if needed, 
  // or just use "JF-Goat" as it is the real GitHub repo name.
  let repoName = 'JF-Goat'; // fallback
  let repoOwner = 'MartinIndra02'; // fallback
  if (tauriConf.plugins && tauriConf.plugins.updater && tauriConf.plugins.updater.endpoints) {
    const endpoint = tauriConf.plugins.updater.endpoints[0];
    const match = endpoint.match(/github\.com\/([^/]+)\/([^/]+)/);
    if (match) {
      repoOwner = match[1];
      repoName = match[2];
    }
  }
  
  // Normalize known renamed repo to target correctly
  if (repoName.toLowerCase() === 'jffast') {
    repoName = 'JF-Goat';
  }

  // GitHub release uploads replace spaces in filenames with dots
  const sanitizedFilename = installerFilename.replace(/ /g, '.');
  const url = `https://github.com/${repoOwner}/${repoName}/releases/download/v${version}/${sanitizedFilename}`;
  console.log(`Installer download URL: ${url}`);

  let platformKey = '';
  if (process.platform === 'win32') {
    platformKey = 'windows-x86_64';
  } else if (process.platform === 'darwin') {
    platformKey = process.arch === 'arm64' ? 'darwin-aarch64' : 'darwin-x86_64';
  } else if (process.platform === 'linux') {
    platformKey = 'linux-x86_64';
  }

  if (!platformKey) {
    console.error(`Unsupported platform: ${process.platform} ${process.arch}`);
    process.exit(1);
  }

  const latestJson = {
    version: version,
    notes: `v${version} Release`,
    pub_date: new Date().toISOString(),
    platforms: {
      [platformKey]: {
        signature: signature,
        url: url
      }
    }
  };

  const outputPath = path.join(bundleDir, 'latest.json');
  fs.writeFileSync(outputPath, JSON.stringify(latestJson, null, 2), 'utf-8');
  console.log(`Successfully generated latest.json at ${outputPath}`);

  const platformOutputPath = path.join(bundleDir, `updater-${platformKey}.json`);
  fs.writeFileSync(platformOutputPath, JSON.stringify(latestJson, null, 2), 'utf-8');
  console.log(`Successfully generated updater-${platformKey}.json at ${platformOutputPath}`);
}

function findFiles(dir, filter) {
  let results = [];
  if (!fs.existsSync(dir)) {
    return results;
  }
  const list = fs.readdirSync(dir);
  for (const file of list) {
    const filePath = path.join(dir, file);
    const stat = fs.statSync(filePath);
    if (stat && stat.isDirectory()) {
      results = results.concat(findFiles(filePath, filter));
    } else if (filter.test(file)) {
      results.push(filePath);
    }
  }
  return results;
}

run();
