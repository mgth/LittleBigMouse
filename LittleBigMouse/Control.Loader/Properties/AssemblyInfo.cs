/*
  LittleBigMouse.Control.Loader
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Control.Loader.

    LittleBigMouse.Control.Loader is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Control.Loader is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using System.Reflection;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Media;

// Les informations générales relatives à un assembly dépendent de 
// l'ensemble d'attributs suivant. Changez les valeurs de ces attributs pour modifier les informations
// associées à un assembly.
[assembly: AssemblyTitle("LittleBigMouse_Control")]
[assembly: AssemblyDescription("DPI Aware LbmMouse control")]
[assembly: AssemblyConfiguration("")]
[assembly: AssemblyCompany("Mgth")]
[assembly: AssemblyProduct("LittleBigMouse_Control")]
[assembly: AssemblyCopyright("Copyright ©  2016")]
[assembly: AssemblyTrademark("")]
[assembly: AssemblyCulture("")]
[assembly: DisableDpiAwareness]

// L'affectation de la valeur false à ComVisible rend les types invisibles dans cet assembly 
// aux composants COM.  Si vous devez accéder à un type dans cet assembly à partir de 
// COM, affectez la valeur true à l'attribut ComVisible sur ce type.
[assembly: ComVisible(false)]

//Pour commencer à générer des applications localisables, définissez 
//<UICulture>CultureUtiliséePourCoder</UICulture> dans votre fichier .csproj
//dans <PropertyGroup>.  Par exemple, si vous utilisez le français
//dans vos fichiers sources, définissez <UICulture> à fr-FR. Puis, supprimez les marques de commentaire de
//l'attribut NeutralResourceLanguage ci-dessous. Mettez à jour "fr-FR" dans
//la ligne ci-après pour qu'elle corresponde au paramètre UICulture du fichier projet.

//[assembly: NeutralResourcesLanguage("en-US", UltimateResourceFallbackLocation.Satellite)]


[assembly: ThemeInfo(
    ResourceDictionaryLocation.None, //où se trouvent les dictionnaires de ressources spécifiques à un thème
                                     //(utilisé si une ressource est introuvable dans la page, 
                                     // ou dictionnaires de ressources de l'application)
    ResourceDictionaryLocation.SourceAssembly //où se trouve le dictionnaire de ressources générique
                                              //(utilisé si une ressource est introuvable dans la page, 
                                              // dans l'application ou dans l'un des dictionnaires de ressources spécifiques à un thème)
)]


// Les informations de version pour un assembly se composent des quatre valeurs suivantes :
//
//      Version principale
//      Version secondaire 
//      Numéro de build
//      Révision
//
// Vous pouvez spécifier toutes les valeurs ou indiquer les numéros de build et de révision par défaut 
// en utilisant '*', comme indiqué ci-dessous :
// [assembly: AssemblyVersion("1.0.*")]
[assembly: AssemblyVersion("4.0.*")]
//[assembly: AssemblyFileVersion("2.0.0.0")]
