class TodoItem extends HTMLElement {
  constructor() {
    super();
  }

  set title(value) {
    this.setAttribute("title", value);
  }

  get title() {
    return this.getAttribute("title");
  }

  set checked(value) {
    this.setAttribute("checked", value);
  }

  get checked() {
    return this.getAttribute("checked");
  }
  connectedCallback() {
    this.render();
  }
  render() {
    const title = this.title;
    const checked = this.checked === "true";

    this.innerHTML = `
      <ul class="list-group list-group-horizontal rounded-0 mb-2">
          <li
            class="list-group-item d-flex align-items-center ps-0 pe-3 py-1 rounded-0 border-0 bg-transparent"
          >
            <div class="form-check">
              <input
                class="form-check-input me-0"
                type="checkbox"
                ${checked ? "checked" : ""}
                id="flexCheckChecked3"
                aria-label="..."
              />
            </div>
          </li>
          <li
            class="list-group-item px-3 py-1 d-flex align-items-center flex-grow-1 border-0 bg-transparent"
          >
            <p
              class="lead fw-normal mb-0 bg-body-tertiary w-100 ms-n2 ps-2 py-1 rounded"
            >
                    ${title}
            </p>
          </li>
  
</ul>
    `;
  }
}
customElements.define("todo-item", TodoItem);
