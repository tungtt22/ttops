import { features } from "./features/registry";
import "./styles.css";

const app = document.querySelector<HTMLElement>("#app");
if (!app) throw new Error("Missing application root");

const sidebar = document.createElement("nav");
sidebar.className = "sidebar";
const content = document.createElement("main");
content.className = "content";
app.replaceChildren(sidebar, content);

for (const feature of features) {
  const button = document.createElement("button");
  button.type = "button";
  button.textContent = feature.label;
  button.onclick = async () => {
    sidebar.querySelectorAll("button").forEach((entry) => entry.classList.remove("active"));
    button.classList.add("active");
    await feature.mount(content);
  };
  sidebar.append(button);
}
sidebar.querySelector<HTMLButtonElement>("button")?.click();
