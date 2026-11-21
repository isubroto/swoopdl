async (page) => {
  const context = await page.context().browser().newContext({
    viewport: { width: 1440, height: 1000 }, colorScheme: 'light', reducedMotion: 'reduce'
  });
  const preview = await context.newPage();
  const failures = [], errors = [];
  preview.on('pageerror', error => errors.push(error.message));
  const check = (condition, label) => { if (!condition) failures.push(label); };
  const visit = async (path, language) => {
    await preview.goto('http://127.0.0.1:8765' + path + '?lang=' + language);
    await preview.waitForFunction(code => document.documentElement.lang === code, language);
    await preview.locator('[data-language-open][role="button"]').waitFor();
  };
  const choose = async language => {
    await preview.locator('[data-language-open]').click();
    await preview.locator('.language-option[lang="' + language + '"]').click();
    await preview.waitForFunction(code => document.documentElement.lang === code, language);
  };
  try {
    await visit('/', 'he');
    const ready = preview.locator('.queue-art > span').first().locator('small');
    check(await ready.evaluate(node => !!node.querySelector('[lang="en"]') || node.getAttribute('lang') === 'en'), 'Partial English has explicit language');
    await choose('ar');
    check(await preview.locator('[data-preview-description]').evaluate(node => !node.hasAttribute('lang')), 'Arabic preview has no stale English language');
    await choose('he');
    check(await preview.locator('[data-preview-description]').getAttribute('lang') === 'en', 'Runtime fallback is marked English');
    await choose('bn');
    check(await preview.locator('h1').textContent() !== 'Big downloads.Small effort.', 'Bengali headline translated');
    await preview.locator('.desktop-nav a[href*="features"]').click();
    await preview.waitForFunction(() => document.documentElement.lang === 'bn');
    check(preview.url().includes('lang=bn'), 'Language retained in navigation');
    await preview.goto('http://127.0.0.1:8765/help/');
    await preview.waitForFunction(() => document.documentElement.lang === 'bn');
    check(await preview.locator('h1').textContent() !== 'A hand whenyou need it.', 'Stored preference retained');
    await preview.locator('[data-language-open]').click();
    check(await preview.locator('.language-option').count() === 86, 'All application languages present');
    await preview.locator('[data-language-search]').fill('no-such-language-123');
    check(await preview.locator('[data-language-empty]').isVisible(), 'Search empty state');
    await preview.keyboard.press('Escape');
    await preview.waitForFunction(() => !document.querySelector('.language-dialog').open);
    check(await preview.locator('[data-language-open]').evaluate(node => document.activeElement === node), 'Picker focus return');
    await preview.keyboard.press('Space');
    check(await preview.locator('.language-dialog').evaluate(node => node.open), 'Keyboard opens picker');
    await preview.keyboard.press('Escape');
    await preview.waitForFunction(() => !document.querySelector('.language-dialog').open);
    await choose('fr');
    check(await preview.locator('html').getAttribute('dir') === 'ltr', 'French direction');
    await choose('en');
    check(await preview.locator('h1').textContent() === 'A hand whenyou need it.', 'English restored');
    let layouts = 0;
    for (const language of ['en', 'bn', 'ar', 'fr', 'de', 'zh-hans']) {
      for (const path of ['/', '/features/', '/download.html', '/themes.html', '/languages.html', '/help/guide-first-download.html', '/privacy.html', '/404.html']) {
        await visit(path, language);
        check(await preview.locator('h1').count() === 1, 'One heading: ' + path + language);
        for (const theme of ['light', 'dark']) {
          await preview.evaluate(value => document.documentElement.dataset.theme = value, theme);
          for (const width of [320, 390, 768, 1440]) {
            await preview.setViewportSize({ width, height: 900 });
            check(await preview.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Overflow: ' + path + ' ' + language + ' ' + width + ' ' + theme);
            layouts++;
          }
        }
      }
    }
    await visit('/', 'en');
    await preview.setViewportSize({ width: 390, height: 844 });
    await preview.locator('.mobile-menu summary').click();
    check(await preview.locator('.mobile-menu').evaluate(node => node.open), 'Mobile menu opens');
    await preview.keyboard.press('Escape');
    check(!await preview.locator('.mobile-menu').evaluate(node => node.open), 'Mobile menu closes');
    await preview.locator('.theme-toggle').click();
    const theme = await preview.locator('html').getAttribute('data-theme');
    await preview.reload();
    check(await preview.locator('html').getAttribute('data-theme') === theme, 'Theme persists');
    return { failures, errors, layoutCases: layouts, languageOptions: 86 };
  } finally {
    await context.close();
  }
}
