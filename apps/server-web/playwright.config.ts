import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  workers: 1,
  use: { baseURL: 'http://127.0.0.1:18787', locale: 'zh-CN', trace: 'retain-on-failure' },
  webServer: [{
    command: 'cargo test --locked -p memedock-server --features web --test auth_devices browser_fixture -- --ignored --exact --nocapture',
    cwd: '../..',
    url: 'http://127.0.0.1:18787/login',
    timeout: 120_000,
    reuseExistingServer: false
  }, {
    command: 'cargo test --locked -p memedock-server --features web --test read_only browser_fixture -- --ignored --exact --nocapture',
    cwd: '../..',
    url: 'http://127.0.0.1:18788/login',
    timeout: 120_000,
    reuseExistingServer: false
  }]
});
