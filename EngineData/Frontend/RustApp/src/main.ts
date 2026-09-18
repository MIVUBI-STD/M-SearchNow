import { mount } from "svelte";
import App from "./App.svelte";
import "./styles/tokens.css";
import "./styles/app.css";
import "./styles/diagnostics.css";
import "./styles/results.css";
import "./styles/content-details.css";
import "./styles/catalog.css";
import "./styles/catalog-modal.css";
import "./styles/presentation.css";

const target = document.getElementById("app");
if (!target) throw new Error("SearchNow app root was not found.");

mount(App, { target });
