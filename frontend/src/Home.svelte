<script>
    import { onMount } from 'svelte';
    import * as utils from './assets/functions.js';
    import {
        endpoints,
        showBasicAuthFields,
        showTokenAuthFields,
        path,
        methods,
        status_code,
        delay,
        rate_limit,
        authType,
        username,
        password,
        tokenData,
        response_file,
        isGraphQL,
        showPathField,
        disableHTTPMethods
    } from './assets/functions.js';

    let endpointsLoader;

    onMount(() => {
        // Configura a referência do loader e carrega os endpoints
        utils.setEndpointsLoader(endpointsLoader);
        utils.loadEndpoints();
    });

    // Observa mudanças no authType
    $: {
        utils.updateAuthFields($authType);
    }

    // Handler para o toggle de GraphQL
    function handleGraphQLChange(event) {
        const isEnabled = event.target.checked;
        utils.handleGraphQLToggle(isEnabled);
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
            <form id="endpoint-form" on:submit={(e) => utils.handleSubmit(
                e, $path, $methods, $status_code, $delay, $rate_limit,
                $authType, $username, $password, $tokenData, $response_file, $isGraphQL
            )}>
                <div class="form-group">
                    <label for="isGraphQL">GraphQL:</label>
                    <div>
                        <input type="checkbox" id="isGraphQL" name="isGraphQL" value="true"
                               bind:checked={$isGraphQL} on:change={handleGraphQLChange}>
                        <label for="isGraphQL">Enable GraphQL support</label>
                    </div>
                </div>

                {#if $showPathField}
                    <div class="form-group">
                        <label for="path">Endpoint Path:</label>
                        <input type="text" id="path" name="path" placeholder="/api/resource" required bind:value={$path}>
                        <small>Start with /api/</small>
                    </div>
                {:else}
                    <div class="form-group info-message">
                        <p>Using default GraphQL endpoint path: <strong>/api/graphql</strong></p>
                    </div>
                {/if}

                <div class="form-group" id="group">
                    <label for="group">HTTP Methods:</label>
                    <div class="checkbox-group">
                        <div>
                            <input type="checkbox" id="get" name="methods" value="GET"
                                   bind:checked={$methods.GET} disabled={$disableHTTPMethods}>
                            <label for="get" class={$disableHTTPMethods ? 'disabled-method' : ''}>GET</label>
                        </div>
                        <div>
                            <input type="checkbox" id="post" name="methods" value="POST"
                                   bind:checked={$methods.POST} disabled={$disableHTTPMethods}>
                            <label for="post" class={$disableHTTPMethods ? 'method-selected' : ''}>POST</label>
                        </div>
                        <div>
                            <input type="checkbox" id="put" name="methods" value="PUT"
                                   bind:checked={$methods.PUT} disabled={$disableHTTPMethods}>
                            <label for="put" class={$disableHTTPMethods ? 'disabled-method' : ''}>PUT</label>
                        </div>
                        <div>
                            <input type="checkbox" id="delete" name="methods" value="DELETE"
                                   bind:checked={$methods.DELETE} disabled={$disableHTTPMethods}>
                            <label for="delete" class={$disableHTTPMethods ? 'disabled-method' : ''}>DELETE</label>
                        </div>
                    </div>
                    {#if $disableHTTPMethods}
                        <small>Only POST method is available for GraphQL</small>
                    {/if}
                </div>

                <div class="form-group">
                    <label for="status-code">Status Code:</label>
                    <input type="number" id="status-code" name="statusCode" min="100" max="599" bind:value={$status_code}>
                </div>

                <div class="form-group">
                    <label for="delay">Response Delay (ms):</label>
                    <input type="number" id="delay" name="delay" min="0" bind:value={$delay}>
                </div>

                <div class="form-group">
                    <label for="rate-limit">Rate Limit (req/min):</label>
                    <input type="text" id="rate-limit" name="rate-limit" min="0" placeholder="e.g (10/60000)" bind:value={$rate_limit}>
                    <small>Set 0 for unlimited</small>
                </div>

                <!-- Authentication Fields -->
                <div class="form-group">
                    <label for="authType">Authentication Type:</label>
                    <select id="authType" name="authType" bind:value={$authType}>
                        <option value="none">No Auth</option>
                        <option value="basic">Basic Auth</option>
                        <option value="token">Token Auth</option>
                    </select>
                </div>

                <!-- Basic Auth Fields -->
                {#if $showBasicAuthFields}
                    <div id="basicAuthFields" class="auth-fields">
                        <h3>Basic Authentication</h3>
                        <label for="username">Username:</label>
                        <input type="text" id="username" name="username" placeholder="Enter username" bind:value={$username}><br><br>
                        <label for="password">Password:</label>
                        <input type="password" id="password" name="password" placeholder="Enter password" bind:value={$password}><br><br>
                    </div>
                {/if}

                <!-- Token Auth Fields -->
                {#if $showTokenAuthFields}
                    <div id="tokenAuthFields" class="auth-fields">
                        <h3>Token Authentication</h3>
                        <label for="tokenData">Token Data (JSON):</label><br>
                        <textarea id="tokenData" name="tokenData" placeholder="{`{\"uat\": \"value\", \"sub\": \"value\", \"iss\": \"value\"}`}" rows="4" cols="50" bind:value={$tokenData}></textarea><br><br>
                    </div>
                {/if}

                <div class="form-group">
                    <label for="response-file">Response JSON File:</label>
                    <input type="file" id="response-file" name="file" accept="application/json" required on:change={utils.handleFileInput}>
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
                    <tbody id="endpoints-list">
                    {#if $endpoints.length === 0}
                        <tr>
                            <td colspan="8" class="empty-state">
                                No endpoints registered. Register your first endpoint using the form.
                            </td>
                        </tr>
                    {:else}
                        {#each $endpoints as endpoint}
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
                                    <input type="checkbox" disabled checked={utils.isAuthenticated(endpoint)}>
                                </td>
                                <td>
                                    <button class="btn-danger" on:click={() => utils.deleteEndpoint(endpoint.path)}>
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