<script>
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
        disableHTTPMethods,
        grpcService,
        grpcRPC,
        setEndpointsLoader,
        loadEndpoints,
        deleteEndpoint,
        handleSubmit,
        handleFileInput,
        updateAuthFields,
        handleGraphQLToggle
    } from './assets/functions.js';

    import { onMount } from 'svelte';

    let authTypeValue = 'none';
    let selectedTab = 'REST';
    

    let loaderRef;

    onMount(() => {
        setEndpointsLoader(loaderRef);
        loadEndpoints();
    });

    $: updateAuthFields(authTypeValue);
    $: handleGraphQLToggle(selectedTab === 'GraphQL');
    $: if (selectedTab === 'gRPC') {
        path.set('/grpc');
        methods.set({ GET: false, POST: true, PUT: false, DELETE: false });
    }
</script>

<div class="container">
    <header>
        <h1>MockiAPI Dashboard</h1>
    </header>

    <main>
        <section class="registration-form">
            <h2>➕ Register New Endpoint</h2>

            <div class="tabs">
                {#each ['REST', 'GraphQL', 'gRPC'] as tab}
                    <button
                            class="tab-btn {selectedTab === tab ? 'tab-active' : ''}"
                            on:click={() => selectedTab = tab}
                    >
                        {tab}
                    </button>
                {/each}
            </div>

            <form on:submit|preventDefault={(e) =>
        handleSubmit(
          e,
          $path,
          $methods,
          $status_code,
          $delay,
          $rate_limit,
          authTypeValue,
          $username,
          $password,
          $tokenData,
          $response_file,
          selectedTab === 'GraphQL',
          $grpcService,
          $grpcRPC
        )
      }>
                {#if selectedTab !== 'gRPC'}
                    {#if $showPathField}
                        <div class="form-group" id="path">
                            <label for="path">Path</label>
                            <input type="text" bind:value={$path} placeholder="/api/example" />
                        </div>
                    {/if}

                    {#if !$disableHTTPMethods}
                        <div class="form-group" id="hMethods">
                            <label for="hMethods">HTTP Methods</label>
                            <div class="checkbox-group">
                                {#each Object.keys($methods) as method}
                                    <div>
                                        <input
                                                type="checkbox"
                                                bind:checked={$methods[method]}
                                                on:change={() => methods.update(m => ({ ...m, [method]: !$methods[method] }))}
                                                id={method}
                                        />
                                        <label for={method}>{method}</label>
                                    </div>
                                {/each}
                            </div>
                        </div>
                    {/if}
                {:else}
                    <div class="form-group" id="service">
                        <label for="service">Service</label>
                        <input type="text" bind:value={$grpcService} placeholder="com.example.MyService" />
                        <input type="hidden" bind:value={$path} placeholder="/api/example" />
                    </div>
                    <div class="form-group" id="rpc">
                        <label for="rpc">RPC Method</label>
                        <input type="text" bind:value={$grpcRPC} placeholder="GetSomething" />
                    </div>
                {/if}

                <div class="form-group" id="status">
                    <label for="status">Status Code</label>
                    <input type="number" bind:value={$status_code} />
                </div>

                <div class="form-group" id="delay">
                    <label for="delay">Delay (ms)</label>
                    <input type="number" bind:value={$delay} />
                </div>

                <div class="form-group" id="limit">
                    <label for="limit">Rate Limit</label>
                    <input type="text" bind:value={$rate_limit} />
                </div>

                <div class="form-group" id="auth">
                    <label for="auth">Authentication Type</label>
                    <select bind:value={authTypeValue}>
                        <option value="none">No Auth</option>
                        <option value="basic">Basic Auth</option>
                        <option value="token">Token Auth</option>
                    </select>
                </div>

                {#if $showBasicAuthFields}
                    <div class="form-group" id="username">
                        <label for="username">Username</label>
                        <input type="text" bind:value={$username} />
                    </div>
                    <div class="form-group" id="pwd">
                        <label for="pwd">Password</label>
                        <input type="password" bind:value={$password} />
                    </div>
                {/if}

                {#if $showTokenAuthFields}
                    <div class="form-group" id="token">
                        <label for="token">Token Data (JSON)</label>
                        <textarea rows="4" bind:value={$tokenData}></textarea>
                    </div>
                {/if}

                <div class="form-group" id="file">
                    <label for="file">Response (JSON File)</label>
                    <input type="file" id="response-file" accept="application/json" on:change={handleFileInput} />
                </div>

                <button class="btn-primary" type="submit">Register</button>
            </form>
        </section>

        <section class="registered-endpoints">
            <h2>📋 Registered Endpoints</h2>
            <div bind:this={loaderRef} class="loader"></div>

            {#if $endpoints.length === 0}
                <div class="empty-state">No endpoints registered yet.</div>
            {:else}
                <table>
                    <thead>
                    <tr>
                        <th>Type</th>
                        <th>Path</th>
                        <th>Methods</th>
                        <th>Status</th>
                        <th>Auth</th>
                        <th>Actions</th>
                    </tr>
                    </thead>
                    <tbody>
                    {#each $endpoints as ep}
                        <tr>
                            <td>{ep.type || 'REST'}</td>
                            <td>{ep.path}</td>
                            <td>
                                {#if ep.method}
                                    <div class="methods-list">
                                        {#each ep.method as m}
                                            <span class="method-tag">{m}</span>
                                        {/each}
                                    </div>
                                {:else}
                                    <span class="disabled-method">—</span>
                                {/if}
                            </td>
                            <td>{ep.status_code}</td>
                            <td>{ep.authentication ? 'Yes' : 'No'}</td>
                            <td>
                                <button class="btn-danger" on:click={() => deleteEndpoint(ep.path)}>Delete</button>
                            </td>
                        </tr>
                    {/each}
                    </tbody>
                </table>
            {/if}
        </section>
    </main>

    <footer>
        MockiAPI &copy; 2025 — REST • GraphQL • gRPC
    </footer>
</div>
