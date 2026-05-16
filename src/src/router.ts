import { createRouter, createWebHistory } from 'vue-router'
import LandingPage from './LandingPage.vue'
import TradingView from './TradingView.vue'

export default createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', component: LandingPage },
    { path: '/trading', component: TradingView },
  ],
})
