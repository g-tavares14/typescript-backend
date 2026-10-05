import { z } from "zod";
import { required } from "./validation.ts";

// Regras de username e email (e a mensagem do 409 de duplicado) compartilhadas entre o cadastro, o login (só o email)
// e a edição do usuário.
// O trim/toLowerCase roda antes da validação do email e do username.
export const emailSchema = z.string(required).trim().toLowerCase().pipe(z.email("Email inválido"));

export const usernameSchema = z
  .string(required)
  .trim()
  .toLowerCase()
  .min(3, "O username deve ter entre 3 e 50 caracteres")
  .max(50, "O username deve ter entre 3 e 50 caracteres")
  // Só a-z, 0-9 e _: barra acento, espaço, letras de outros alfabetos e caracteres invisíveis.
  // Junto com o toLowerCase, "Joao" e "joao" viram o mesmo username (o UNIQUE do banco pega o duplicado).
  // Vem depois dos checks de tamanho, então um username curto continua recebendo a mensagem de tamanho.
  .regex(/^[a-z0-9_]+$/, "O username só pode ter letras sem acento, números e _");

// Regra da senha nova (cadastro e troca de senha). No login e nas confirmações não há tamanho mínimo: contas antigas
// com senhas menores continuariam conseguindo entrar se a regra mudar.
export const passwordSchema = z.string(required).min(8, "A senha deve ter no mínimo 8 caracteres");

// Resposta do 409 quando username ou email já pertencem a outra conta (cadastro e edição do usuário).
// O banco não diz qual dos dois colidiu, e a mensagem também não: uma só para os dois casos.
export const DUPLICATE_USER = "Email ou username já cadastrado";
