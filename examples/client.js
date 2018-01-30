const ws = new WebSocket("ws://127.0.0.1:8080");

const send_message = () => {
  ws.send("Hello world");
}

ws.onopen = () => {
  console.info("connection opened");
  setTimeout(send_message, 5);
};

ws.onmessage = (msg) => {
  console.info("received message:", msg);
};

ws.onclose = () => {
  console.info("websocket connection is closed");
};
