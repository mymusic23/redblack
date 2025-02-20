<template>
  <div class="wasm-test">
    <h4>WASM Test</h4>
    <div class="test-results">
      <p>Add Result (5 + 3): {{ addResult }}</p>
      <p>Multiply Result (4 * 6): {{ multiplyResult }}</p>
      <p>Greet Result: {{ greetResult }}</p>
    </div>
  </div>
</template>

<script>
import { initWasm } from '../wasm/test';

export default {
  name: 'WasmTest',
  data() {
    return {
      addResult: null,
      multiplyResult: null,
      greetResult: null
    }
  },
  async mounted() {
    try {
      const { add, multiply, greet } = await initWasm();
      
      // Test WASM functions
      this.addResult = add(5, 3);
      this.multiplyResult = multiply(4, 6);
      this.greetResult = greet("WASM");
      
      console.log('WASM functions tested successfully');
    } catch (error) {
      console.error('WASM test error:', error);
    }
  }
}
</script>

<style scoped>
.wasm-test {
  margin: 20px 0;
  padding: 20px;
  background-color: #191a19;
  border-radius: 8px;
}

.test-results {
  margin-bottom: 20px;
}
</style>
