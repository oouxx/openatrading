<template>
  <div class="min-h-screen bg-bg">
    <!-- Global Nav -->
    <header class="fixed top-0 w-full z-50 bg-black h-11">
      <div class="max-w-[1600px] mx-auto px-6 h-full flex items-center justify-between">
        <div class="flex items-center gap-6 flex-shrink-0">
          <router-link to="/" class="text-nav-link font-semibold text-white tracking-tight hover:text-ai transition-colors">
            OpenATrading
          </router-link>
          <router-link to="/trading" class="text-nav-link font-mono text-ink-muted hover:text-white transition-colors">
            交易终端
          </router-link>
          <router-link to="/roles" class="text-nav-link font-mono text-ink-muted hover:text-white transition-colors">
            角色策略
          </router-link>
        </div>
      </div>
    </header>

    <router-view :key="$route.fullPath" />
  </div>
</template>
