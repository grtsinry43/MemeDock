import { test, expect, type Page } from '@playwright/test';

test.use({ baseURL: 'http://127.0.0.1:18788' });

async function login(page: Page) {
  await page.goto('/library/');
  await expect(page).toHaveURL(/\/login$/);
  await page.getByLabel('用户名').fill('admin');
  await page.getByLabel('密码', { exact: true }).fill('memedock-test-password');
  await page.getByRole('button', { name: '登录', exact: true }).click();
  await expect(page.getByRole('heading', { name: '图库', exact: true })).toBeVisible();
}

async function tapBrand(page: Page) {
  const brand = page.getByRole('link', { name: /MemeDock/ }).first();
  for (let tap = 0; tap < 7; tap += 1) {
    await brand.click();
  }
}

test('seven brand taps reveal and dismiss the easter egg', async ({ page }) => {
  await login(page);
  await tapBrand(page);
  const toast = page.getByRole('status').filter({ hasText: '小秘密' });
  await expect(toast).toBeVisible();
  await page.getByRole('button', { name: '关闭', exact: true }).click();
  await expect(toast).toHaveCount(0);
});

test.describe('reduced motion', () => {
  test.use({ reducedMotion: 'reduce' });

  test('easter egg appears without an entrance animation', async ({ page }) => {
    await login(page);
    await tapBrand(page);
    const toast = page.getByRole('status').filter({ hasText: '小秘密' });
    await expect(toast).toBeVisible();
    expect(await toast.evaluate(element => getComputedStyle(element).animationName)).toBe('none');
  });
});
