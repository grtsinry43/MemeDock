<script lang="ts">
  import { Images, Languages } from '@lucide/svelte';
  import { DropdownMenu } from 'bits-ui';
  import LoginForm from './LoginForm.svelte';
  import { page } from '$app/state';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';
  import Menu from '#lib/ui/primitives/DropdownMenu.svelte';
</script>

<main class="relative flex min-h-dvh items-center justify-center px-5 py-12">
  <div class="absolute right-4 top-4">
    <Menu label={i18n.t.login.switchLanguage}>
      {#snippet trigger()}
        <Languages size={17} />
        <span class="text-caption">
          {i18n.preference === 'system' ? i18n.t.settings.languageSystem : i18n.preference === 'zh' ? '中文' : 'English'}
        </span>
      {/snippet}
      <DropdownMenu.Item class="menu-item" onclick={() => i18n.set('system')}>
        {i18n.t.settings.languageSystem}
      </DropdownMenu.Item>
      <DropdownMenu.Item class="menu-item" onclick={() => i18n.set('zh')}>
        {i18n.t.settings.languageZh}
      </DropdownMenu.Item>
      <DropdownMenu.Item class="menu-item" onclick={() => i18n.set('en')}>
        {i18n.t.settings.languageEn}
      </DropdownMenu.Item>
    </Menu>
  </div>
  <div class="w-full max-w-sm">
    <div class="mb-9 flex items-center gap-3">
      <span class="flex size-12 items-center justify-center rounded-panel bg-foreground text-on-inverse">
        <Images size={25} />
      </span>
      <span class="text-section font-semibold">
        MemeDock
        <span class="block text-caption font-normal text-secondary">{i18n.t.login.brandTag}</span>
      </span>
    </div>
    <section class="panel p-7">
      <h1 class="text-title font-semibold">{i18n.t.login.title}</h1>
      <p class="mb-7 mt-2 text-body text-secondary">{i18n.t.login.description}</p>
      {#if page.url.searchParams.has('passwordChanged')}
        <p role="status" class="mb-5 text-body text-success">{i18n.t.login.passwordChanged}</p>
      {/if}
      <LoginForm />
    </section>
    <p class="mt-6 text-center text-caption leading-5 text-secondary">{i18n.t.login.adminHint}</p>
  </div>
</main>
