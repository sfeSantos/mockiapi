<script>
    import { onMount } from 'svelte';

    // State variables
    let endpointForm;
    let endpointsList;
    let endpointsLoader;
    let endpoints = [];
    let showBasicAuthFields = false;
    let showTokenAuthFields = false;

    // Form data
    let path = '';
    let methods = {
        GET: true,
        POST: false,
        PUT: false,
        DELETE: false
    };
    let status_code;
    let delay;
    let rate_limit = '';
    let authType = 'none';
    let username = '';
    let password = '';
    let tokenData = '';
    let response_file;

    // Load all registered endpoints on mount
    onMount(() => {
        loadEndpoints();
    });

    // Handle form submission
    function handleSubmit(e) {
        e.preventDefault();

        const formData = new FormData();
        formData.append("path", path);
        formData.append("methods", extractMethods(methods));
        formData.append("status_code", status_code);
        formData.append("delay", delay);
        formData.append("rate_limit", rate_limit);
        formData.append("authentication", handleAuthentication())

        if (response_file) {
            formData.append('file', response_file);
        } else {
            showNotification('Please select a JSON file', 'error');
            return;
        }

        // Register endpoint
        registerEndpoint(formData);
    }

    // Get selected methods
    function extractMethods(methodsSelected) {
        const selectedMethods = [];

        Object.entries(methodsSelected).forEach(([method, selected]) => {
            if (selected) {
                selectedMethods.push(method);
            }
        });

        if (selectedMethods.length === 0) {
            showNotification('Please select at least one HTTP method', 'error');
            return;
        }

        return selectedMethods.join(",");
    }

    // Add authentication
    function handleAuthentication() {
        let authentication = null;

        if (authType === 'basic') {
            authentication = {
                username,
                password
            };
        } else if (authType === 'token') {
            try {
                authentication = JSON.parse(tokenData);
            } catch (e) {
                showNotification('Invalid token data format.', 'error');
                return;
            }
        }

        return JSON.stringify(authentication);
    }

    // Register new endpoint
    async function registerEndpoint(formData) {
        showLoader(true);

        try {
            const response = await fetch('/register', {
                method: 'POST',
                body: formData
            });

            if (!response.ok) {
                throw new Error('Failed to register endpoint');
            }

            await response.json();
            showNotification('Endpoint registered successfully!', 'success');
            resetForm();
            await loadEndpoints();
        } catch (error) {
            showNotification(error.message, 'error');
        } finally {
            showLoader(false);
        }
    }

    // Reset form fields
    function resetForm() {
        path = '';
        methods = { GET: true, POST: false, PUT: false, DELETE: false };
        status_code = 200;
        delay = 0;
        rate_limit = '';
        authType = 'none';
        username = '';
        password = '';
        tokenData = '';
        response_file = null;

        // Reset the file input (needs to be handled separately in Svelte)
        const fileInput = document.getElementById('response-file');
        if (fileInput) fileInput.value = '';
    }

    // Load all registered endpoints
    async function loadEndpoints() {
        showLoader(true);

        try {
            const response = await fetch('/list');
            const data = await response.json();

            // Convert object to an array
            endpoints = Object.entries(data).map(([path, config]) => ({
                path,
                ...config
            }));
        } catch (error) {
            showNotification('Failed to load endpoints', 'error');
        } finally {
            showLoader(false);
        }
    }

    // Delete endpoint
    async function deleteEndpoint(path) {
        if (confirm(`Are you sure you want to delete the endpoint "${path}"?`)) {
            showLoader(true);

            try {
                const response = await fetch(`/admin/endpoints/${encodeURIComponent(path)}`, {
                    method: 'DELETE'
                });

                if (!response.ok) {
                    throw new Error('Failed to delete endpoint');
                }

                showNotification('Endpoint deleted successfully!', 'success');
                await loadEndpoints();
            } catch (error) {
                showNotification(error.message, 'error');
            } finally {
                showLoader(false);
            }
        }
    }

    // Show notification
    function showNotification(message, type) {
        const notification = document.createElement('div');
        notification.className = `notification ${type}`;
        notification.textContent = message;

        document.querySelector('.container').prepend(notification);

        setTimeout(() => {
            notification.remove();
        }, 5000);
    }

    // Show/hide loader
    function showLoader(show) {
        if (endpointsLoader) {
            endpointsLoader.style.display = show ? 'block' : 'none';
        }
    }

    // Check if the endpoint is authenticated
    function isAuthenticated(endpoint) {
        return endpoint.authentication !== null && endpoint.authentication !== undefined;
    }

    // Handler for file input
    function handleFileInput(e) {
        const files = e.target.files;
        if (files.length > 0) {
            response_file = files[0];
        }
    }

    // Watch for changes in authType
    $: {
        showBasicAuthFields = authType === 'basic';
        showTokenAuthFields = authType === 'token';
    }
</script>

<div class="container">
    <header>
        <h1>MockiAPI</h1>
        <p>Register mock API endpoints and their responses</p>
    </header>

    <main>
        <section class="registration-form">
            <h2>Register New Endpoint</h2>
            <form id="endpoint-form" on:submit={handleSubmit} bind:this={endpointForm}>
                <div class="form-group">
                    <label for="path">Endpoint Path:</label>
                    <input type="text" id="path" name="path" placeholder="/api/resource" required bind:value={path}>
                    <small>Start with /api/</small>
                </div>

                <div class="form-group" id="group">
                    <label for="group">HTTP Methods:</label>
                    <div class="checkbox-group">
                        <div>
                            <input type="checkbox" id="get" name="methods" value="GET" bind:checked={methods.GET}>
                            <label for="get">GET</label>
                        </div>
                        <div>
                            <input type="checkbox" id="post" name="methods" value="POST" bind:checked={methods.POST}>
                            <label for="post">POST</label>
                        </div>
                        <div>
                            <input type="checkbox" id="put" name="methods" value="PUT" bind:checked={methods.PUT}>
                            <label for="put">PUT</label>
                        </div>
                        <div>
                            <input type="checkbox" id="delete" name="methods" value="DELETE" bind:checked={methods.DELETE}>
                            <label for="delete">DELETE</label>
                        </div>
                    </div>
                </div>

                <div class="form-group">
                    <label for="status-code">Status Code:</label>
                    <input type="number" id="status-code" name="statusCode" min="100" max="599" bind:value={status_code}>
                </div>

                <div class="form-group">
                    <label for="delay">Response Delay (ms):</label>
                    <input type="number" id="delay" name="delay" min="0" bind:value={delay}>
                </div>

                <div class="form-group">
                    <label for="rate-limit">Rate Limit (req/min):</label>
                    <input type="text" id="rate-limit" name="rate-limit" min="0" placeholder="e.g (10/60000)" bind:value={rate_limit}>
                    <small>Set 0 for unlimited</small>
                </div>

                <!-- Authentication Fields -->
                <div class="form-group">
                    <label for="authType">Authentication Type:</label>
                    <select id="authType" name="authType" bind:value={authType}>
                        <option value="none">No Auth</option>
                        <option value="basic">Basic Auth</option>
                        <option value="token">Token Auth</option>
                    </select>
                </div>

                <!-- Basic Auth Fields -->
                {#if showBasicAuthFields}
                    <div id="basicAuthFields" class="auth-fields">
                        <h3>Basic Authentication</h3>
                        <label for="username">Username:</label>
                        <input type="text" id="username" name="username" placeholder="Enter username" bind:value={username}><br><br>
                        <label for="password">Password:</label>
                        <input type="password" id="password" name="password" placeholder="Enter password" bind:value={password}><br><br>
                    </div>
                {/if}

                <!-- Token Auth Fields -->
                {#if showTokenAuthFields}
                    <div id="tokenAuthFields" class="auth-fields">
                        <h3>Token Authentication</h3>
                        <label for="tokenData">Token Data (JSON):</label><br>
                        <textarea id="tokenData" name="tokenData" placeholder="{`{\"uat\": \"value\", \"sub\": \"value\", \"iss\": \"value\"}`}" rows="4" cols="50" bind:value={tokenData}></textarea><br><br>
                    </div>
                {/if}

                <div class="form-group">
                    <label for="response-file">Response JSON File:</label>
                    <input type="file" id="response-file" name="file" accept="application/json" required on:change={handleFileInput}>
                </div>

                <div class="form-group">
                    <button type="submit" class="btn-primary">Register Endpoint</button>
                </div>
            </form>
        </section>

        <section class="registered-endpoints">
            <h2>Registered Endpoints</h2>
            <div class="loader" id="endpoints-loader" bind:this={endpointsLoader}></div>
            <div id="endpoints-container">
                <table id="endpoints-table">
                    <thead>
                    <tr>
                        <th>Path</th>
                        <th>Methods</th>
                        <th>File</th>
                        <th>Status</th>
                        <th>Delay (ms)</th>
                        <th>Rate Limit req/min</th>
                        <th>Authenticated</th>
                        <th>Actions</th>
                    </tr>
                    </thead>
                    <tbody id="endpoints-list" bind:this={endpointsList}>
                    {#if endpoints.length === 0}
                        <tr>
                            <td colspan="8" class="empty-state">
                                No endpoints registered. Register your first endpoint using the form.
                            </td>
                        </tr>
                    {:else}
                        {#each endpoints as endpoint}
                            <tr>
                                <td>{endpoint.path}</td>
                                <td>
                                    <div class="methods-list">
                                        {#each endpoint.methods as method}
                                            <span class="method-tag">{method}</span>
                                        {/each}
                                    </div>
                                </td>
                                <td>{endpoint.file.split('/')[1]}</td>
                                <td>{endpoint.statusCode}</td>
                                <td>{endpoint.delay} ms</td>
                                {#if endpoint.rate_limit}
                                    <td>{endpoint.rate_limit.requests}/{endpoint.rate_limit.window_ms}</td>
                                {:else}
                                    <td>---</td>
                                {/if}
                                <td align="center">
                                    <input type="checkbox" disabled checked={isAuthenticated(endpoint)}>
                                </td>
                                <td>
                                    <button class="btn-danger" on:click={() => deleteEndpoint(endpoint.path)}>
                                        Delete
                                    </button>
                                </td>
                            </tr>
                        {/each}
                    {/if}
                    </tbody>
                </table>
            </div>
        </section>
    </main>

    <footer>
        <p>MockiAPI - Created by <a href="mailto:eduf.santos@mail.com">Eduardo Santos</a></p>
    </footer>
</div>
