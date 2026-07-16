import { mount } from "svelte";
import App from "./App.svelte";
import "./lib/theme.css";

const target = document.getElementById("app");
if (target === null) {
  throw new Error("missing #app mount point in index.html");
}

const app = mount(App, { target });

export default app;
