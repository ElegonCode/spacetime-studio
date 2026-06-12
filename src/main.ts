import "./assets/css/main.css";

import { createApp } from "vue";
import { router } from "./router";
import ui from "@nuxt/ui/vue-plugin";
import App from "./App.vue";

const app = createApp(App);

app.use(router);
app.use(ui);

app.mount("#app");
