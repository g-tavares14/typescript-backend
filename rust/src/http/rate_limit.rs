// Rate limit por IP, em memória (o equivalente ao @fastify/rate-limit). Contra força bruta e contra o consumo de
// memória do argon2 (64 MiB por hash).
//
// Em vez de uma layer (que rodaria ANTES dos extractors e contaria requisições sem token), o limite é um extractor:
// cada rota decide onde ele entra na lista de parâmetros. Login e cadastro: antes do corpo (corpo inválido também
// conta). Rotas de /users/me: depois do CurrentUser (sem token → 401, sem consumir o limite).
//
// Algoritmo: GCRA (crate governor). "5 por minuto" = até 5 seguidas, e depois uma nova a cada 12 s. Diferente da
// janela fixa do Fastify (5 e depois espera o minuto virar); ver "Diferenças para o front" na spec.
use std::{marker::PhantomData, net::IpAddr, net::SocketAddr, num::NonZeroU32};

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;
use governor::{
    DefaultKeyedRateLimiter, Quota, RateLimiter,
    clock::{Clock, DefaultClock},
};

use crate::{http::error::AppError, state::AppState};

type Limiter = DefaultKeyedRateLimiter<IpAddr>;

// Um contador por rota (cada um com a sua cota), todos por IP.
pub struct RateLimits {
    enabled: bool,
    login: Limiter,
    register: Limiter,
    update_me: Limiter,
    delete_me: Limiter,
    change_password: Limiter,
}

fn per_minute(max: u32) -> Limiter {
    // NonZeroU32: o tipo garante que a cota nunca é 0. `expect` só aqui, com valores fixos no código.
    RateLimiter::keyed(Quota::per_minute(
        NonZeroU32::new(max).expect("cota maior que zero"),
    ))
}

impl RateLimits {
    pub fn new(enabled: bool) -> Self {
        RateLimits {
            enabled,
            login: per_minute(5),
            register: per_minute(3),
            update_me: per_minute(10),
            delete_me: per_minute(5),
            change_password: per_minute(5),
        }
    }

    // Esquece os IPs que já recuperaram a cota inteira. Sem isto, a memória cresceria com cada IP novo
    // (o main.rs chama a cada minuto).
    pub fn retain_recent(&self) {
        for limiter in [
            &self.login,
            &self.register,
            &self.update_me,
            &self.delete_me,
            &self.change_password,
        ] {
            limiter.retain_recent();
        }
    }
}

// Qual contador uma rota usa. Cada rota tem um tipo "marcador" (struct sem campos) que implementa esta trait.
pub trait LimitedRoute {
    fn limiter(limits: &RateLimits) -> &Limiter;
}

pub struct Login;
pub struct Register;
pub struct UpdateMe;
pub struct DeleteMe;
pub struct ChangePassword;

impl LimitedRoute for Login {
    fn limiter(limits: &RateLimits) -> &Limiter {
        &limits.login
    }
}
impl LimitedRoute for Register {
    fn limiter(limits: &RateLimits) -> &Limiter {
        &limits.register
    }
}
impl LimitedRoute for UpdateMe {
    fn limiter(limits: &RateLimits) -> &Limiter {
        &limits.update_me
    }
}
impl LimitedRoute for DeleteMe {
    fn limiter(limits: &RateLimits) -> &Limiter {
        &limits.delete_me
    }
}
impl LimitedRoute for ChangePassword {
    fn limiter(limits: &RateLimits) -> &Limiter {
        &limits.change_password
    }
}

// O extractor. `RateLimited<Login>` num handler = "conte esta requisição no limite do login".
// PhantomData<R>: a struct não guarda nenhum R; o tipo só existe para o compilador escolher o contador.
pub struct RateLimited<R>(PhantomData<R>);

impl<R: LimitedRoute + Send> FromRequestParts<AppState> for RateLimited<R> {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if !state.limits.enabled {
            return Ok(RateLimited(PhantomData));
        }
        // IP de quem conectou (o main.rs serve com `into_make_service_with_connect_info`). Atrás de um proxy
        // (ex.: Cloudflare) este seria o IP do proxy para todo mundo: aí é preciso ler o IP real de um header
        // confiável, o equivalente ao trustProxy do Fastify.
        let ConnectInfo(address) = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .ok_or_else(|| {
                AppError::internal("ConnectInfo ausente: o servidor precisa de connect_info")
            })?;

        match R::limiter(&state.limits).check_key(&address.ip()) {
            Ok(()) => Ok(RateLimited(PhantomData)),
            Err(not_until) => {
                let wait = not_until.wait_time_from(DefaultClock::default().now());
                // Retry-After em segundos inteiros, arredondado para cima (nunca 0).
                Err(AppError::TooManyRequests {
                    retry_after_seconds: wait.as_secs() + 1,
                })
            }
        }
    }
}
