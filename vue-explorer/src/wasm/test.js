// import init, { add, multiply, greet } from './redgold_gui';
//
// export const initWasm = async () => {
//     try {
//         // Initialize the WASM module
//         await init();
//
//         // Test the functions
//         const sum = add(5, 3);
//         const product = multiply(4, 6);
//         const greeting = greet("WASM");
//
//         console.log('WASM Test - Addition:', sum);
//         console.log('WASM Test - Multiplication:', product);
//         console.log('WASM Test - Greeting:', greeting);
//
//         return {
//             testConstant: "WASM module loaded successfully!",
//             add,
//             multiply,
//             greet
//         };
//     } catch (e) {
//         console.error("Failed to load WASM module:", e);
//         throw e;
//     }
// };
