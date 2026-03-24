import { readFile } from 'node:fs/promises'
import { resolve } from 'node:path'

const filePath = resolve(process.cwd(), 'src/App.tsx')
const source = await readFile(filePath, 'utf8')
const externalLinkPattern = /<a\s+[^>]*href="https?:\/\/[^"]+"[^>]*target="_blank"[^>]*>/g
const relPattern = /rel="([^"]+)"/
const failures = []

for (const match of source.matchAll(externalLinkPattern)) {
  const anchor = match[0]
  const rel = anchor.match(relPattern)?.[1] ?? ''
  const tokens = new Set(rel.split(/\s+/).filter(Boolean))

  if (!tokens.has('noopener') || !tokens.has('noreferrer')) {
    failures.push(anchor)
  }
}

if (failures.length > 0) {
  console.error('External links opened in a new tab must include rel="noopener noreferrer".')
  for (const failure of failures) {
    console.error(`- ${failure}`)
  }
  process.exit(1)
}
