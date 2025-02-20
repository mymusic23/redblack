import "bootstrap/dist/css/bootstrap.min.css"
import "bootstrap"

import {createApp} from 'vue'
import {createRouter, createWebHistory} from 'vue-router'
import App from './App.vue'
// import HelloWorld from './components/HelloWorld.vue'
// import HelloWorld2 from './components/HelloWorld2.vue'
import Dashboard from './components/DashboardScreen.vue'
// import Header from './components/Header.vue'
import HashDetails from "./components/HashDetails.vue";
import store from './store';
import FaucetRequest from "@/components/FaucetRequest.vue";
import PoolParties from "@/components/PoolParties.vue"; // Assuming store.js is in the root directory alongside main.js

// Define routes
const routes = [
    { path: '/hash/:param', component: HashDetails },
    { path: '/', component: Dashboard},
    { path: '/faucet', component: FaucetRequest},
    { path: '/parties', component: PoolParties},
]

// Create router
const router = createRouter({
    history: createWebHistory(),
    routes
})

// Create app
const app = createApp(App)

// Initialize WASM before mounting the app
const initializeWasm = async () => {
    try {
        // const { initWasm } = await import('./wasm/test');
        // const { testConstant, wasm } = await initWasm();
        // console.log('WASM Test:', testConstant);
        // // Make wasm instance available globally and in store
        // window.redgoldWasm = wasm;
        // store.state.wasm = wasm;
        // Mount app after WASM is loaded
        app.use(router)
        app.use(store)
        app.mount('#app')
    } catch (error) {
        console.error('Failed to initialize WASM:', error);
        // Mount app anyway to show error state
        app.use(router)
        app.use(store)
        app.mount('#app')
    }
};

initializeWasm();
