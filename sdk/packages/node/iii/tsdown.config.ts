import { defineConfig } from 'tsdown'

export default defineConfig({
  entry: {
    index: './src/index.ts',
    stream: './src/stream.ts',
    state: './src/state.ts',
    helpers: './src/helpers.ts',
    types: './src/public-types.ts',
  },
  format: ['esm', 'cjs'],
  dts: true,
  sourcemap: true,
  clean: true,
  minify: false,
  treeshake: true,
  deps: { neverBundle: [] },
})
