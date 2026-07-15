const { chmod, lstat, readdir } = require('node:fs/promises')
const { join } = require('node:path')

exports.default = async function makeElectronCopyWritable({ appOutDir }) {
  await makeWritable(appOutDir)
}

async function makeWritable(path) {
  const info = await lstat(path)
  if (info.isSymbolicLink()) return
  await chmod(path, info.mode | 0o200)
  if (!info.isDirectory()) return
  for (const name of await readdir(path)) {
    await makeWritable(join(path, name))
  }
}
