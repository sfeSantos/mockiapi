// endpointLogic.js
import { writable, derived } from 'svelte/store';

// Stores para os estados
export const endpoints = writable([]);
export const showBasicAuthFields = writable(false);
export const showTokenAuthFields = writable(false);

// Stores para dados do formulário
export const path = writable('');
export const methods = writable({
    GET: true,
    POST: false,
    PUT: false,
    DELETE: false
});
export const status_code = writable();
export const delay = writable();
export const rate_limit = writable('');
export const with_dynamic_vars = writable(false);
export const authType = writable('none');
export const username = writable('');
export const password = writable('');
export const tokenData = writable('');
export const response_file = writable(null);
export const isGraphQL = writable(false);
export const grpcRPC = writable('');
export const grpcService = writable('');

// Derived stores para controle de UI com base no estado GraphQL
// Como isGraphQL começa como false, showPathField deve começar como true
export const showPathField = derived(isGraphQL, $isGraphQL => !$isGraphQL);
export const disableHTTPMethods = derived(isGraphQL, $isGraphQL => $isGraphQL);

// Referência para o loader
let endpointsLoaderRef;

export function setEndpointsLoader(ref) {
    endpointsLoaderRef = ref;
}

// Carrega todos os endpoints registrados
export async function loadEndpoints() {
    showLoader(true);

    try {
        const response = await fetch('/list');
        const data = await response.json();

        // Converte objeto para array
        endpoints.set(Object.entries(data).map(([path, config]) => ({
            path,
            ...config
        })));
    } catch (error) {
        showNotification('Failed to load endpoints', 'error');
    } finally {
        showLoader(false);
    }
}

// Extrai métodos selecionados
export function extractMethods(methodsSelected) {
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

// Processa a autenticação
export function handleAuthentication(authTypeValue, usernameValue, passwordValue, tokenDataValue) {
    let authenticationValue = null;

    if (authTypeValue === 'basic') {
        authenticationValue = {
            username: usernameValue,
            password: passwordValue
        };
    } else if (authTypeValue === 'token') {
        try {
            authenticationValue = JSON.parse(tokenDataValue);
        } catch (e) {
            showNotification('Invalid token data format.', 'error');
            return;
        }
    }

    return JSON.stringify(authenticationValue);
}

// Registra um novo endpoint
export async function registerEndpoint(formData) {
    showLoader(true);

    try {
        const response = await fetch('/register', {
            method: 'POST',
            body: formData
        });

        if (!response.ok) {
            throw new Error(await registrationFailure(response));
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

// The server answers a refused form with the reason as a JSON string
async function registrationFailure(response) {
    try {
        const reason = await response.json();

        if (typeof reason === 'string') {
            return reason;
        }
    } catch (error) {
        // not JSON, fall back to the generic message
    }

    return 'Failed to register endpoint';
}

// Deleta um endpoint
export async function deleteEndpoint(pathToDelete) {
    if (confirm(`Are you sure you want to delete the endpoint "${pathToDelete}"?`)) {
        showLoader(true);

        try {
            const response = await fetch(`/delete/${encodeURIComponent(pathToDelete)}`, {
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

// Reseta o formulário
export function resetForm() {
    path.set('');
    methods.set({ GET: true, POST: false, PUT: false, DELETE: false });
    status_code.set(null);
    delay.set(null);
    rate_limit.set('');
    with_dynamic_vars.set(false);
    authType.set('none');
    username.set('');
    password.set('');
    tokenData.set('');
    response_file.set(null);
    isGraphQL.set(false);
    grpcRPC.set('')

    // Reset do input de arquivo
    const fileInput = document.getElementById('response-file');
    if (fileInput) fileInput.value = '';
}

// Exibe notificação
export function showNotification(message, type) {
    const notification = document.createElement('div');
    notification.className = `notification ${type}`;
    notification.textContent = message;

    document.querySelector('.container').prepend(notification);

    setTimeout(() => {
        notification.remove();
    }, 5000);
}

// Exibe/esconde o loader
export function showLoader(show) {
    if (endpointsLoaderRef) {
        endpointsLoaderRef.style.display = show ? 'block' : 'none';
    }
}

// Verifica se o endpoint está autenticado
export function isAuthenticated(endpoint) {
    return endpoint.authentication !== null && endpoint.authentication !== undefined;
}

// Processa o envio do formulário
export function handleSubmit(event, pathValue, methodsValue, status_codeValue, delayValue,
                             rate_limitValue, authTypeValue, usernameValue, passwordValue,
                             tokenDataValue, response_fileValue, isGraphQLValue, grpcService, grpcRPC,
                             withDynamicVarsValue) {
    event.preventDefault();

    // Both helpers notify the user and return undefined when the input is not usable
    const selectedMethods = extractMethods(methodsValue);
    const authentication = handleAuthentication(authTypeValue, usernameValue, passwordValue, tokenDataValue);

    if (selectedMethods === undefined || authentication === undefined) {
        return;
    }

    const formData = new FormData();

    // Se for GraphQL, use um caminho padrão
    if (isGraphQLValue) {
        formData.append("path", "/api/graphql");
    } else {
        formData.append("path", pathValue);
    }

    formData.append("methods", selectedMethods);
    formData.append("status_code", status_codeValue);
    formData.append("delay", delayValue);
    formData.append("rate_limit", rate_limitValue);
    formData.append("authentication", authentication);
    formData.append("with_dynamic_vars", Boolean(withDynamicVarsValue).toString());
    formData.append("isGraphQL", isGraphQLValue.toString());
    formData.append("grpcService", grpcService);
    formData.append("grpcRPC", grpcRPC);

    if (response_fileValue) {
        formData.append('file', response_fileValue);
    } else {
        showNotification('Please select a JSON file', 'error');
        return;
    }

    // Registra o endpoint
    registerEndpoint(formData);
}

// Handler para input de arquivo
export function handleFileInput(event) {
    const files = event.target.files;
    if (files.length > 0) {
        response_file.set(files[0]);
    }
}

// Atualiza o estado dos campos de autenticação quando o tipo de auth muda
export function updateAuthFields(newAuthType) {
    showBasicAuthFields.set(newAuthType === 'basic');
    showTokenAuthFields.set(newAuthType === 'token');
}

// Handler para o toggle do GraphQL
export function handleGraphQLToggle(isEnabled) {
    if (isEnabled) {
        methods.update(_ => ({
            GET: false,
            POST: true,
            PUT: false,
            DELETE: false
        }));
        path.set('/api/graphql');
    } else {
        methods.update(_ => ({
            GET: true,
            POST: false,
            PUT: false,
            DELETE: false
        }));
        path.set('');
    }
}