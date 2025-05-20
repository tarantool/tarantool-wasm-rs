export const incomingHandler = {
    handle: (request) => {
        console.log("Handling incoming request", request);
        return { status: 200 };
    }
};

export const run = {
    run: async function() {
        console.info("Hello from JS WASM!")
    }
}   
