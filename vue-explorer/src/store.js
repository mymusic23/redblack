// store.js
import {createStore} from 'vuex';

export default createStore({
    state: {
        btcExchangeRate: 30000.0, // Default value
        ethExchangeRate: 2000.0, // Default value
        wasm: null, // WASM module instance
        wasmError: null // Any WASM loading errors
    },
    getters: {
        // Getter for btcExchangeRate
        getBtcExchangeRate: (state) => {
            return state.btcExchangeRate;
        },
        getEthExchangeRate: (state) => {
            return state.ethExchangeRate;
        },
        getWasm: (state) => {
            return state.wasm;
        },
        getWasmError: (state) => {
            return state.wasmError;
        }
    },
    mutations: {
        // Mutation (setter) for btcExchangeRate
        setBtcExchangeRate(state, rate) {
            state.btcExchangeRate = rate;
        },
        setEthExchangeRate(state, rate) {
            state.ethExchangeRate = rate;
        },
        setWasm(state, instance) {
            state.wasm = instance;
            state.wasmError = null;
        },
        setWasmError(state, error) {
            state.wasmError = error;
            state.wasm = null;
        }
    },
    actions: {
        // // Optional: Async action that could fetch and then commit the new rate
        // async fetchAndSetBtcExchangeRate({ commit }) {
        //     // Example using a fictional API endpoint
        //     // const response = await axios.get('https://api.example.com/btcRate');
        //     // commit('setBtcExchangeRate', response.data.rate);
        // }
    }
});
