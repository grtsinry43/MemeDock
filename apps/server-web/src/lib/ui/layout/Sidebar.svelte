<script lang="ts">
  import { Images, Laptop, Settings, ArrowRightLeft, History } from '@lucide/svelte';
  import type { Snippet } from 'svelte';
  import { i18n } from '#lib/shared/i18n/i18n.svelte.ts';

  let { pathname, account }: { pathname: string; account: Snippet } = $props();

  const navItems = [
    { href: '/library', label: () => i18n.t.nav.library, icon: Images },
    { href: '/sync', label: () => i18n.t.nav.sync, icon: ArrowRightLeft },
    { href: '/activity', label: () => i18n.t.nav.activity, icon: History },
    { href: '/devices', label: () => i18n.t.nav.devices, icon: Laptop },
    { href: '/settings', label: () => i18n.t.nav.settings, icon: Settings },
  ];
</script>

<div class="grid grid-cols-[minmax(0,1fr)_auto] items-center md:contents">
  <a href="/library" class="order-1 flex min-w-0 items-center gap-3 px-2 py-3 font-semibold">
    <span class="flex size-10 shrink-0 items-center justify-center rounded-control bg-foreground text-on-inverse">
      <Images size={22} />
    </span>
    <span class="text-section">
      {i18n.t.nav.serviceName}
      <span class="mt-0.5 block text-caption font-normal text-secondary">{i18n.t.nav.serviceTag}</span>
    </span>
  </a>
  <nav
    aria-label={i18n.t.nav.mainNavLabel}
    class="order-3 col-span-2 mt-3 flex min-w-0 gap-1 overflow-x-auto pb-1 md:order-2 md:mt-5 md:flex-col md:gap-2 md:overflow-visible md:pb-0"
  >
    {#each navItems as item}
      <a
        href={item.href}
        aria-current={pathname === item.href ? 'page' : undefined}
        class={[
          'flex min-h-11 shrink-0 items-center gap-3 whitespace-nowrap rounded-control px-3 text-body transition-colors md:flex-none',
          pathname === item.href ? 'bg-input font-semibold text-foreground' : 'text-secondary hover:bg-input',
        ]}
      >
        <item.icon size={18} />
        {item.label()}
      </a>
    {/each}
  </nav>
  <div class="order-2 min-w-0 md:order-3 md:mt-auto md:border-t md:border-separator md:pt-3">
    {@render account()}
  </div>
</div>
