import { readdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { constants, gzipSync } from 'node:zlib'

const COMPRESSIBLE = /\.(html|js|css|svg|json)$/

/**
 * Writes a `.gz` next to every text file of the build output.
 *
 * The server embeds the whole output directory and sends the `.gz` to clients that accept
 * gzip, so nothing is compressed while a request is being served.
 */
export function precompress() {
  let outputDirectory

  return {
    name: 'precompress',
    apply: 'build',

    configResolved(config) {
      outputDirectory = resolve(config.root, config.build.outDir)
    },

    // index.html and the files copied from public/ are only on disk at this point.
    closeBundle() {
      const files = readdirSync(outputDirectory, { recursive: true })
        .filter((file) => COMPRESSIBLE.test(file))

      for (const file of files) {
        const path = join(outputDirectory, file)
        const original = readFileSync(path)
        const compressed = gzipSync(original, { level: constants.Z_BEST_COMPRESSION })

        if (compressed.length < original.length) {
          writeFileSync(`${path}.gz`, compressed)
        }
      }
    }
  }
}
