import { createRouter, createWebHistory } from 'vue-router'
import LandingPage from './LandingPage.vue'
import TradingView from './TradingView.vue'
import RoleView from './RoleView.vue'
import AiCopilot from './AiCopilot.vue'
import ReplayView from './ReplayView.vue'

const isTauri = '__TAURI_INTERNALS__' in window

export default createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', component: isTauri ? TradingView : LandingPage },
    { path: '/trading', component: TradingView },
    { path: '/roles', component: RoleView },
    { path: '/copilot', component: AiCopilot },
    { path: '/replay', component: ReplayView },
  ],
})