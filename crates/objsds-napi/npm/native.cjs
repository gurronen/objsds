'use strict'

const fs = require('node:fs')
const path = require('node:path')

function detectLibc() {
  if (process.platform !== 'linux') return undefined
  try {
    return process.report?.getReport()?.header?.glibcVersionRuntime ? 'gnu' : 'musl'
  } catch {
    return undefined
  }
}

function targetNames() {
  if (process.platform === 'darwin') {
    if (process.arch !== 'arm64') {
      throw new Error('Intel macOS is not supported; @objsds/client requires Apple Silicon')
    }
    return ['darwin-arm64']
  }
  if (process.platform !== 'linux') return [`${process.platform}-${process.arch}`]
  const detected = detectLibc()
  return detected === 'musl'
    ? [`linux-${process.arch}-musl`, `linux-${process.arch}-gnu`]
    : [`linux-${process.arch}-gnu`, `linux-${process.arch}-musl`]
}

function load() {
  const explicit = process.env.OBJSDS_NATIVE_BINARY
  if (explicit) {
    if (!explicit.endsWith('.node')) {
      throw new Error(`OBJSDS_NATIVE_BINARY must point to a .node file, got: ${explicit}`)
    }
    return require(path.resolve(explicit))
  }

  const errors = []
  for (const target of targetNames()) {
    for (const candidate of [
      `@objsds/client-${target}`,
      path.join(__dirname, `objsds-napi.${target}.node`),
      path.join(__dirname, 'objsds-napi.node'),
    ]) {
      try {
        if (candidate.endsWith('.node') && !fs.existsSync(candidate)) continue
        return require(candidate)
      } catch (error) {
        if (error?.code === 'MODULE_NOT_FOUND' || error?.code === 'ERR_DLOPEN_FAILED') {
          errors.push(error)
          continue
        }
        throw error
      }
    }
  }

  const error = new Error(
    `Unable to load the objsds native binding for ${process.platform}-${process.arch}. ` +
      'Install the matching platform package or set OBJSDS_NATIVE_BINARY.',
  )
  if (errors.length > 0) error.cause = errors[0]
  throw error
}

module.exports = load()
