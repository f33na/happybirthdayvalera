import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'node',
    include: ['accuracy/**/*.test.ts'],
    // The first run downloads the model; transcribing on the CPU of a CI runner takes a while too.
    testTimeout: 15 * 60_000,
    hookTimeout: 15 * 60_000,
    fileParallelism: false,
  },
});
